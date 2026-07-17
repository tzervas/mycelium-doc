//! **Inline-span parsing** for prose / heading / table-cell text (a CommonMark *subset*, the same
//! "honestly a subset, named as one" discipline as [`crate::corpus`]'s block parser). The corpus
//! parser is block-only, so `**strong**`, `*em*`/`_em_`, inline `` `code` ``, and `[text](url)` reach
//! the doc-IR as **verbatim text** — which then renders as literal `**`/backticks, defeating the
//! comfortable-reading goal. This module parses that text into inline [`Span`]s **at render time**;
//! each emitter ([`crate::emit::html`], [`crate::emit::typst`]) renders the spans to its own markup.
//!
//! ## Render-time, never an IR node (parity-safe)
//! A [`Span`] is a *view* over the node's existing text, **not** a doc-IR node — so it never perturbs
//! a node's content address (ADR-003): the IR keeps the verbatim text, its `data-cid` is unchanged,
//! and the machine JSON view still serialises the raw text. HTML and Typst therefore stay two views
//! of the *same* content-addressed nodes (the §4.1 dual-projection-parity lint holds). The parse is
//! shared here (one grammar); only the rendering differs per emitter.
//!
//! ## The subset, and its honest limits
//! Priority, high→low: inline code `` ` `` (verbatim — `*`/`_` inside backticks are **not** emphasis,
//! per the requirement "inline code first"), then `**strong**`, then `*em*` / `_em_`, then
//! `[text](url)`. `_` does **not** open emphasis intra-word (so `file_index` and `snake_case` in prose
//! are never mangled — the CommonMark left/right-flanking rule, important for this technical corpus).
//! Unbalanced markers are left as literal text (never dropped, never a panic). Recursion is
//! depth-capped ([`MAX_DEPTH`]); beyond it the remainder is literal text (never-silent, no overflow).
//! HTML-escaping is the **emitter's** job, applied per-span *after* parsing (so a `<` in prose is
//! escaped inside the `<strong>`, never interpreted).

/// The maximum inline nesting depth parsed; beyond it the remainder is emitted as literal text (a
/// bound against adversarial input — never a stack overflow, never dropped text).
pub const MAX_DEPTH: u8 = 8;

/// One inline span — a render-time view over a slice of the node's text (never an IR node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span<'a> {
    /// A literal text run (the emitter escapes it).
    Text(&'a str),
    /// Inline `` `code` `` — verbatim content (no nested parsing; the emitter escapes it).
    Code(&'a str),
    /// `**strong**` — bold, with parsed inner spans.
    Strong(Vec<Span<'a>>),
    /// `*em*` / `_em_` — emphasis, with parsed inner spans.
    Em(Vec<Span<'a>>),
    /// `[text](href)` — a link; `text` is parsed, `href` is the raw target (the emitter decides how
    /// to treat internal vs external — internal navigation is owned by the resolved `Xref` nodes).
    Link {
        /// The parsed link text.
        text: Vec<Span<'a>>,
        /// The raw href/target (a `"title"` suffix already stripped).
        href: &'a str,
    },
}

/// Parse `text` into inline spans (the subset in the module docs). Total over the input: every byte
/// appears in exactly one span's text (never dropped, never duplicated).
#[must_use]
pub fn parse(text: &str) -> Vec<Span<'_>> {
    parse_spans(text, 0)
}

fn parse_spans(text: &str, depth: u8) -> Vec<Span<'_>> {
    let mut out = Vec::new();
    if depth >= MAX_DEPTH {
        if !text.is_empty() {
            out.push(Span::Text(text));
        }
        return out;
    }
    let b = text.as_bytes();
    let mut i = 0;
    let mut lit = 0; // start byte of the pending literal run
    while i < b.len() {
        let matched = match b[i] {
            b'`' => match_code(text, i),
            b'*' if b.get(i + 1) == Some(&b'*') => match_strong(text, i, depth),
            b'*' => match_em(text, i, b'*', depth),
            b'_' => match_em(text, i, b'_', depth),
            b'[' => match_link(text, i, depth),
            _ => None,
        };
        if let Some((span, end)) = matched {
            if lit < i {
                out.push(Span::Text(&text[lit..i]));
            }
            out.push(span);
            i = end;
            lit = end;
        } else {
            i = advance(text, i);
        }
    }
    if lit < b.len() {
        out.push(Span::Text(&text[lit..]));
    }
    out
}

/// The next char boundary at/after `i` (UTF-8 safe single-char advance).
fn advance(text: &str, i: usize) -> usize {
    text[i..].chars().next().map_or(i + 1, |c| i + c.len_utf8())
}

/// The byte index just past the run of consecutive `delim` bytes starting at `start` (all ASCII).
fn delim_run_end(b: &[u8], start: usize, delim: u8) -> usize {
    let mut k = start;
    while k < b.len() && b[k] == delim {
        k += 1;
    }
    k
}

/// Inline code `` `...` `` — verbatim; no nested parsing (so `*`/`_` inside are not emphasis).
fn match_code(text: &str, i: usize) -> Option<(Span<'_>, usize)> {
    let rel = text[i + 1..].find('`')?;
    let close = i + 1 + rel;
    if close == i + 1 {
        return None; // empty `` — leave the backticks literal
    }
    Some((Span::Code(&text[i + 1..close]), close + 1))
}

/// `**strong**` — the inner is parsed; a code span inside is skipped when scanning for the closer.
fn match_strong(text: &str, i: usize, depth: u8) -> Option<(Span<'_>, usize)> {
    let inner_start = i + 2;
    let close = find_closer(text, inner_start, "**")?;
    let inner = &text[inner_start..close];
    if inner.is_empty() || inner.starts_with(' ') || inner.ends_with(' ') {
        return None;
    }
    Some((Span::Strong(parse_spans(inner, depth + 1)), close + 2))
}

/// `*em*` / `_em_` — single-delimiter emphasis with flanking rules; `_` never opens intra-word.
fn match_em(text: &str, i: usize, delim: u8, depth: u8) -> Option<(Span<'_>, usize)> {
    let b = text.as_bytes();
    // Opener must be followed by a non-space (left-flanking).
    let next = *b.get(i + 1)?;
    if next.is_ascii_whitespace() {
        return None;
    }
    // `_` does not open emphasis intra-word (protects snake_case / file_index in prose).
    if delim == b'_' && i > 0 && b[i - 1].is_ascii_alphanumeric() {
        return None;
    }
    // Find the closing delimiter, skipping any code span so a `*`/`_` inside backticks is not a closer.
    let mut j = i + 1;
    while j < b.len() {
        if b[j] == b'`' {
            if let Some(rel) = text[j + 1..].find('`') {
                j = j + 1 + rel + 1;
                continue;
            }
        }
        if b[j] == delim {
            // A run of >=2 delimiters is a strong (or longer) marker, not a single-emphasis closer —
            // skip it whole so em-containing-strong (`*a **b** c*`) nests correctly instead of
            // closing on one `*` of the `**` (which would leave a stray marker).
            let run = delim_run_end(b, j, delim);
            if run - j >= 2 {
                j = run;
                continue;
            }
            let prev = b[j - 1];
            // A valid closer is preceded by a non-space, and (for `_`) is not intra-word either.
            let closer_ok = !(prev.is_ascii_whitespace()
                || delim == b'_' && b.get(j + 1).is_some_and(u8::is_ascii_alphanumeric));
            if closer_ok {
                let inner = &text[i + 1..j];
                if inner.is_empty() {
                    return None;
                }
                return Some((Span::Em(parse_spans(inner, depth + 1)), j + 1));
            }
        }
        j = advance(text, j);
    }
    None
}

/// `[text](href)` — text is parsed, href is the raw target (a `"title"` suffix stripped). The closing
/// `]` is matched with **bracket-depth tracking**, so link text may itself contain balanced brackets
/// (`[List[0]](url)` → text `List[0]`); genuinely unbalanced bracket text finds no matching `]` and is
/// left fully literal rather than cut at the wrong bracket (G2 — never mis-parse).
fn match_link(text: &str, i: usize, depth: u8) -> Option<(Span<'_>, usize)> {
    let close_br = matching_bracket(text, i)?;
    if text.as_bytes().get(close_br + 1) != Some(&b'(') {
        return None;
    }
    let href_start = close_br + 2;
    let close_paren_rel = text[href_start..].find(')')?;
    let close_paren = href_start + close_paren_rel;
    let link_text = &text[i + 1..close_br];
    // Strip a `"title"` suffix (as `crate::corpus::extract_links` does).
    let href = text[href_start..close_paren]
        .split_whitespace()
        .next()
        .unwrap_or("");
    Some((
        Span::Link {
            text: parse_spans(link_text, depth + 1),
            href,
        },
        close_paren + 1,
    ))
}

/// The byte index of the `]` that matches the `[` at `open` (bracket-depth balanced), or `None` if
/// unbalanced. Inline code spans are skipped so a `[`/`]` inside backticks does not perturb the depth.
fn matching_bracket(text: &str, open: usize) -> Option<usize> {
    let b = text.as_bytes();
    let mut depth = 0i32;
    let mut j = open;
    while j < b.len() {
        match b[j] {
            b'`' => {
                if let Some(rel) = text[j + 1..].find('`') {
                    j = j + 1 + rel + 1;
                    continue;
                }
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
        j = advance(text, j);
    }
    None
}

/// The byte offset of the closing `delim` (a 2-char marker like `**`) at/after `from`, skipping any
/// inline code span so a marker inside backticks is not a closer. `None` if unbalanced.
fn find_closer(text: &str, from: usize, delim: &str) -> Option<usize> {
    let b = text.as_bytes();
    let dbytes = delim.as_bytes();
    let mut j = from;
    while j + dbytes.len() <= b.len() {
        if b[j] == b'`' {
            if let Some(rel) = text[j + 1..].find('`') {
                j = j + 1 + rel + 1;
                continue;
            }
        }
        if &b[j..j + dbytes.len()] == dbytes {
            return Some(j);
        }
        j = advance(text, j);
    }
    None
}

/// Whether a link `href` is an external target the inline renderer should link directly. Internal /
/// relative links are owned by the resolved `Xref` sibling nodes (the inline renderer emits only the
/// link *text* for them, so it never produces a dead link to a raw `.md` path).
#[must_use]
pub fn is_external(href: &str) -> bool {
    href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:")
}

/// Strip inline markdown from `text`, returning **plain, unescaped** text — formatting markers and
/// link syntax removed, code/link text kept. For nav labels / a `<title>` / a JSON search field where
/// literal `**`/backticks would leak (the emitter HTML-escapes for an HTML context; a JSON context is
/// escaped by the serializer). E.g. ``"DN-102 · The `?` Try-Operator"`` → `"DN-102 · The ? Try-Operator"`.
#[must_use]
pub fn to_plain(text: &str) -> String {
    fn walk(spans: &[Span<'_>], out: &mut String) {
        for span in spans {
            match span {
                Span::Text(t) | Span::Code(t) => out.push_str(t),
                Span::Strong(inner) | Span::Em(inner) => walk(inner, out),
                Span::Link { text, .. } => walk(text, out),
            }
        }
    }
    let mut out = String::new();
    walk(&parse(text), &mut out);
    out
}
