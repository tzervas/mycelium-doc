//! **Lexical syntax highlighting** for `myc`/`myc-checked` code examples (wired into
//! [`crate::emit::html`]'s `Example` arm). Derived from the *trusted L1 lexer*
//! ([`mycelium_l1::lexer::lex_with_comments`]) — no new dependency, no re-lexing: each token is
//! wrapped in a `<span class="tok-…">` so the shared theme ([`crate::theme`]) can colour it.
//!
//! ## Scope & honesty (`Empirical/Declared`)
//! Classification is **purely lexical / token-kind** — exactly the honest scope of the LSP
//! semantic-tokens provider (`mycelium-lsp`), whose buckets this mirrors *locally* (this crate stays
//! disjoint from `mycelium-lsp`; the small map is reimplemented here, not shared). Because the lexer
//! has no semantic context, an identifier renders plain (default ink) **unless** it is immediately
//! followed by `(` — a single lexical fn-position heuristic that promotes it to `tok-fn`; no other
//! function/binding distinction is available (VR-5: never presented as type-aware). The `classify`
//! match is **exhaustive over every [`Tok`] variant** (no wildcard): a future lexer token that is not
//! mapped here fails to **compile** rather than silently rendering unhighlighted — the same
//! never-silent posture the LSP provider documents (G2).
//!
//! ## Never-silent fallback (G2)
//! [`highlight`] returns `Some(html)` **only** when it can honestly highlight: the language is
//! `myc`/`myc-checked`, the source is ASCII (so scalar columns are exact — the same UTF-16 stopgap
//! the LSP span layer takes), and the lexer accepts it. On *any* other case — a different language, a
//! lexer error, or a non-ASCII span — it returns `None` and the caller emits the plain escaped
//! source. It **never** fabricates or partially-highlights: the reconstructed text is byte-for-byte
//! the escaped original (tokens wrapped, nothing added or dropped), so a code block is always the
//! real source, coloured or plain — never a lie.

use mycelium_l1::lexer::lex_with_comments;
use mycelium_l1::token::Tok;

/// The CSS class (a `tok-*` suffix from the reviewed design system in [`crate::theme::READING_CSS`])
/// for a lexed token, or `None` for tokens that carry no highlight — delimiters (`()[]{}`, `:` `,`
/// `;` `.`), plain identifiers, and `Eof` — which are emitted as plain (default-ink) text, an
/// explicit choice, never a silent drop.
///
/// The design system's seven code classes are `tok-kw` · `tok-type` · `tok-num` · `tok-str` ·
/// `tok-com` · `tok-fn` · `tok-op`. This lexical layer maps to six of them here; the seventh,
/// `tok-fn`, is assigned by [`highlight`]'s fn-position heuristic (an identifier immediately followed
/// by `(`), since the lexer cannot otherwise tell a function name from a binding (VR-5). The
/// guarantee-strength members (`Exact/Proven/Empirical/Declared`) fold into `tok-kw` — they are
/// reserved keyword vocabulary, and the design's 7-class palette has no separate member class (FLAG:
/// a per-member lattice colouring — amber `Empirical`, clay `Declared` — is a possible enhancement).
///
/// **Exhaustive on purpose** (no `_` arm): an unmapped future token is a compile error, not a silent
/// gap (G2).
fn classify(tok: &Tok) -> Option<&'static str> {
    let class = match tok {
        // Declaration + control + runtime-vocabulary keywords, plus the guarantee-strength members
        // (Exact/Proven/Empirical/Declared — reserved keyword vocabulary, folded here per the note).
        Tok::Nodule
        | Tok::Phylum
        | Tok::Colony
        | Tok::Hypha
        | Tok::Fuse
        | Tok::Mesh
        | Tok::Graft
        | Tok::Cyst
        | Tok::Xloc
        | Tok::Forage
        | Tok::Backbone
        | Tok::Tier
        | Tok::Reclaim
        | Tok::Consume
        | Tok::Grow
        | Tok::Derive
        | Tok::Use
        | Tok::Pub
        | Tok::Priv
        | Tok::Type
        | Tok::Trait
        | Tok::Impl
        | Tok::Fn
        | Tok::Matured
        | Tok::Thaw
        | Tok::Let
        | Tok::In
        | Tok::If
        | Tok::Then
        | Tok::Else
        | Tok::Match
        | Tok::For
        | Tok::Swap
        | Tok::Default
        | Tok::Paradigm
        | Tok::With
        | Tok::Wild
        | Tok::Spore
        | Tok::Wrapping
        | Tok::To
        | Tok::Policy
        | Tok::Lambda
        | Tok::Object
        | Tok::Via
        | Tok::Lower
        | Tok::Strength(_) => "tok-kw",
        // Substrate / representation types and scalars (incl. M-915 short forms + RFC-0032 repr types).
        Tok::Binary
        | Tok::Ternary
        | Tok::Dense
        | Tok::Vsa
        | Tok::BinShort
        | Tok::TernShort
        | Tok::EmbShort
        | Tok::HvecShort
        | Tok::Seq
        | Tok::Bytes
        | Tok::Float
        | Tok::Substrate
        | Tok::Sparse
        | Tok::Scalar(_) => "tok-type",
        // Numeric-shaped literals (binary/ternary/int/byte-string/float).
        Tok::BinLit(_) | Tok::TritLit(_) | Tok::Int(_) | Tok::BytesLit(_) | Tok::FloatLit(_) => {
            "tok-num"
        }
        // Textual string literals.
        Tok::StrLit(_) => "tok-str",
        // Operators / annotations / arrows / comparisons / shifts.
        Tok::Plus
        | Tok::Minus
        | Tok::Star
        | Tok::Slash
        | Tok::Percent
        | Tok::Question
        | Tok::Caret
        | Tok::Amp
        | Tok::AmpAmp
        | Tok::Eq
        | Tok::EqEq
        | Tok::Arrow
        | Tok::FatArrow
        | Tok::Bang
        | Tok::BangEq
        | Tok::Pipe
        | Tok::PipePipe
        | Tok::At
        | Tok::AtStdSys
        | Tok::LAngle
        | Tok::RAngle
        | Tok::Shl
        | Tok::Shr => "tok-op",
        // Identifiers render plain (default ink) unless the fn-position heuristic in `highlight`
        // promotes them to `tok-fn`; delimiters and `Eof` never highlight — a documented choice.
        // `::` (DN-113 Rank 1 / M-1060, the cross-phylum `use dep::a.b.Item` boundary marker) joins
        // `:` in this delimiter bucket.
        Tok::Ident(_)
        | Tok::LParen
        | Tok::RParen
        | Tok::LBrace
        | Tok::RBrace
        | Tok::LBracket
        | Tok::RBracket
        | Tok::Colon
        | Tok::ColonColon
        | Tok::Comma
        | Tok::Semi
        | Tok::Dot
        | Tok::Eof => return None,
    };
    Some(class)
}

/// Is `lang` a Mycelium fence we highlight? (`myc` or `myc-checked` — both go through the L1 lexer.)
#[must_use]
pub fn is_myc_lang(lang: &str) -> bool {
    matches!(lang, "myc" | "myc-checked")
}

/// One lexical item resolved to a single-line char span, plus its highlight class (`None` = plain).
struct Item {
    line: u32,
    col: u32,
    len: u32,
    class: Option<&'static str>,
}

/// Highlight `source` as Mycelium, or `None` to signal the caller should emit plain escaped text.
///
/// Returns `Some(html)` where each classified token is wrapped in `<span class="tok-…">…</span>` and
/// everything else (whitespace, delimiters, comments-are-`tok-com`) is preserved verbatim
/// (escaped). The stripped-of-tags text equals `html_escape(source)` exactly — never fabricated,
/// never partial (G2). `None` on: a non-`myc` language, a non-ASCII source, or a lexer error.
#[must_use]
pub fn highlight(lang: &str, source: &str) -> Option<String> {
    if !is_myc_lang(lang) || !source.is_ascii() {
        return None;
    }
    let (toks, comments) = lex_with_comments(source).ok()?;

    // Per-line char lengths (a trailing '\r' from CRLF is dropped so it never inflates a token).
    let line_len: Vec<u32> = source
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).chars().count() as u32)
        .collect();
    let len_of = |line: u32| -> u32 {
        line.checked_sub(1)
            .and_then(|z| line_len.get(z as usize).copied())
            .unwrap_or(0)
    };

    // Merge tokens (minus Eof) and comments into one source-ordered list of raw starts. ALL tokens
    // are kept (delimiters too) so a token's length is bounded by the *next* lexical boundary, even
    // though delimiters render plain (class `None`).
    struct Raw {
        line: u32,
        col: u32,
        class: Option<&'static str>,
        comment: bool,
    }
    let mut raws: Vec<Raw> = Vec::with_capacity(toks.len() + comments.len());
    for (j, s) in toks.iter().enumerate() {
        if matches!(s.tok, Tok::Eof) {
            continue;
        }
        // Fn-position heuristic (lexical, Declared): an identifier immediately followed by `(` is a
        // function name/call → `tok-fn`; every other identifier renders plain (default ink). This is
        // the only fn/binding distinction available without semantic context (VR-5).
        let class = if matches!(&s.tok, Tok::Ident(_))
            && matches!(toks.get(j + 1).map(|n| &n.tok), Some(Tok::LParen))
        {
            Some("tok-fn")
        } else {
            classify(&s.tok)
        };
        raws.push(Raw {
            line: s.pos.line,
            col: s.pos.col,
            class,
            comment: false,
        });
    }
    for c in &comments {
        raws.push(Raw {
            line: c.line,
            col: c.col,
            class: Some("tok-com"),
            comment: true,
        });
    }
    raws.sort_by_key(|r| (r.line, r.col));

    // Resolve each raw to a length (next-boundary method — a comment runs to end-of-line; a token to
    // the next raw on its line, trailing whitespace trimmed).
    let mut items: Vec<Item> = Vec::with_capacity(raws.len());
    for i in 0..raws.len() {
        let r = &raws[i];
        let len = if r.comment {
            len_of(r.line).saturating_sub(r.col.saturating_sub(1))
        } else {
            let raw_end = match raws.get(i + 1) {
                Some(next) if next.line == r.line => next.col,
                _ => len_of(r.line) + 1,
            };
            trimmed_len(source, r.line, r.col, raw_end)
        };
        if len == 0 {
            continue;
        }
        items.push(Item {
            line: r.line,
            col: r.col,
            len,
            class: r.class,
        });
    }

    Some(reconstruct(source, &items))
}

/// The trimmed char length of a token occupying `[col, raw_end)` (1-based, exclusive) on `line` —
/// the raw span minus trailing whitespace separating it from the next boundary. Recomputed from the
/// source line's chars (exact for ASCII).
fn trimmed_len(source: &str, line: u32, col: u32, raw_end: u32) -> u32 {
    let Some(text) = source
        .split('\n')
        .nth(line.saturating_sub(1) as usize)
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
    else {
        return 0;
    };
    let chars: Vec<char> = text.chars().collect();
    let start = col.saturating_sub(1) as usize;
    let end = (raw_end.saturating_sub(1) as usize).min(chars.len());
    if end <= start {
        return 0;
    }
    let mut e = end;
    while e > start && chars[e - 1].is_whitespace() {
        e -= 1;
    }
    (e - start) as u32
}

/// Walk `source` char-by-char, wrapping each classified [`Item`]'s run in a span and emitting
/// everything else (gaps, delimiters, unclassified tokens) as plain escaped text. Faithful by
/// construction: every source char is emitted exactly once, escaped — so stripping the tags yields
/// `html_escape(source)` (G2: never fabricated, never partial).
fn reconstruct(source: &str, items: &[Item]) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + items.len() * 24);
    let mut i = 0usize; // char index
    let mut line = 1u32;
    let mut col = 1u32;
    let mut it = 0usize; // next item index (items are sorted by (line, col))
    while i < chars.len() {
        // Skip any items we've walked past (defensive — should not happen with sorted, in-range items).
        while it < items.len() && (items[it].line, items[it].col) < (line, col) {
            it += 1;
        }
        if it < items.len() && items[it].line == line && items[it].col == col {
            let item = &items[it];
            let take = (item.len as usize).min(chars.len() - i);
            let open_close = item.class.is_some();
            if let Some(cls) = item.class {
                out.push_str("<span class=\"");
                out.push_str(cls);
                out.push_str("\">");
            }
            for &c in &chars[i..i + take] {
                push_escaped(&mut out, c);
            }
            if open_close {
                out.push_str("</span>");
            }
            i += take;
            col += take as u32;
            it += 1;
        } else {
            let c = chars[i];
            push_escaped(&mut out, c);
            i += 1;
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
    }
    out
}

/// Escape one char into `out` (the same rules as [`crate::emit::html_escape`], per-char to avoid a
/// temporary allocation on the hot loop).
fn push_escaped(out: &mut String, c: char) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&#39;"),
        _ => out.push(c),
    }
}

/// Test-only helper: the concatenated *text* of the highlighted HTML (tags stripped) — the property
/// that must equal `html_escape(source)`. Kept here so the in-crate test (`src/tests/highlight.rs`)
/// can assert faithfulness without duplicating the tag-strip.
#[cfg(test)]
#[must_use]
pub(crate) fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}
