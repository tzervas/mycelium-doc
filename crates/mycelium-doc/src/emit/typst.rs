//! The Typst projection (spec §8.1/§8.2 — Typst is the ratified PDF engine). Renders the doc-IR to a
//! single `.typ` source; the actual PDF compile (`typst compile`) is an *optional* downstream step
//! that **skips gracefully when the `typst` binary is absent** (the env may lack it) — never a
//! half-build. Each block is preceded by a `// cid:` comment so the Typst view shares identity with
//! the HTML/JSON views (one content-addressed truth).
//!
//! **Print code legibility (§8.2).** Captured code blocks are tuned for *paper*, with a different
//! scale than the web theme: the body prose is ~10.5pt, but fenced/raw code is set ~0.82× body with
//! tighter leading so lines fit without wrapping, inside a **light tinted box with a hairline border**
//! (never a filled dark panel — print-ink-friendly), comfortable page margins, light-only (print has
//! no dark mode). Honest scope: the PDF path renders code as monospace **without** the web
//! highlighter's per-token colours (that would need per-token `#text(fill:)` emission or a bundled
//! theme — future work), so there is nothing to mis-colour.

use crate::inline::{self, Span};
use crate::ir::{DocModel, Node, Payload};

/// Render parsed inline [`Span`]s to Typst markup: `#strong[…]`, `#emph[…]`, `#raw("…")` for inline
/// code, and `#link("…")[…]` for **external** links only (internal/relative links render as their
/// text — their resolved navigation is the `Xref` node, so the inline path never emits a broken PDF
/// link to a raw `.md` path). Function forms are used throughout so code/link content with Typst
/// metacharacters is robust (never a broken `.typ`).
///
/// Each inline **call** (`#raw`/`#strong`/`#emph`/`#link`) is followed by a zero-width space
/// ([`CALL_SEP`]): in Typst markup a `(` or `[` immediately after a call is parsed as a *curried
/// call* on its result (so a projected `` `math`(f64) `` → `#raw("math")(f64)` would be a call, not
/// text — a `typst compile` failure). The zero-width break terminates the call so the following char
/// is literal text, while remaining invisible in the PDF (G2: every `.typ` compiles, output unchanged).
fn render_inline_typst(spans: &[Span<'_>]) -> String {
    let mut out = String::new();
    for span in spans {
        match span {
            Span::Text(t) => out.push_str(&escape(t)),
            Span::Code(c) => {
                out.push_str(&format!("#raw(\"{}\"){CALL_SEP}", escape_str(c)));
            }
            Span::Strong(inner) => {
                out.push_str("#strong[");
                out.push_str(&render_inline_typst(inner));
                out.push_str(&format!("]{CALL_SEP}"));
            }
            Span::Em(inner) => {
                out.push_str("#emph[");
                out.push_str(&render_inline_typst(inner));
                out.push_str(&format!("]{CALL_SEP}"));
            }
            Span::Link { text, href } => {
                if inline::is_external(href) {
                    out.push_str(&format!("#link(\"{}\")[", escape_str(href)));
                    out.push_str(&render_inline_typst(text));
                    out.push_str(&format!("]{CALL_SEP}"));
                } else {
                    out.push_str(&render_inline_typst(text));
                }
            }
        }
    }
    out
}

/// The zero-width space appended after each inline Typst call to terminate a possible curried-call
/// chain (see [`render_inline_typst`]). Invisible in the rendered PDF.
const CALL_SEP: &str = "\u{200b}";

/// Parse + render inline markdown in `text` to Typst markup (the common one-shot).
fn inline_typst(text: &str) -> String {
    render_inline_typst(&inline::parse(text))
}

/// Render a whole prose block to Typst, **line by line**, so the emitted `.typ` always compiles.
///
/// Two robustness invariants, both never-silent (G2 — every `.typ` compiles):
/// - **Per-line inline parsing.** Prose carries embedded newlines (soft wraps / the rows of a
///   projected pipe-table joined by the block parser). Parsing the *whole* block risks an unbalanced
///   backtick/marker pairing across lines (inverting text↔code and colliding Typst's `[]`/`""`
///   delimiters). Parsing each line independently bounds any unbalanced marker to that one line
///   (where it degrades to literal text), so the delimiters stay balanced.
/// - **Leading-markup guard.** Typst reads block markup only at a LINE START — `/ ` (description-list
///   term), `- `/`+ ` (bullets), `= ` (heading), `N.`/`N)` (ordered list). A rendered line beginning
///   with one is prefixed with a zero-width `#h(0pt)` so the trigger is no longer at line start and
///   Typst reads it as literal text. Our own inline markup (`#strong[…]` etc.) starts with `#` — never
///   a trigger char — so it is left untouched.
fn render_prose_typst(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let rendered = inline_typst(line);
        if line_starts_block_markup(rendered.trim_start()) {
            out.push_str("#h(0pt)");
        }
        out.push_str(&rendered);
    }
    out
}

/// Whether a (leading-whitespace-trimmed) line begins with a Typst block-markup trigger.
fn line_starts_block_markup(t: &str) -> bool {
    let b = t.as_bytes();
    match b.first() {
        // `/ ` `- ` `+ ` `= ` (and multi-`=` headings): the marker char, then a space or another
        // marker. A bare `-3` or `/x` mid-sentence never reaches here (only line starts do), and
        // guarding it is harmless anyway.
        Some(b'/' | b'-' | b'+' | b'=') => true,
        // Ordered list: one or more digits then `.` or `)`.
        Some(d) if d.is_ascii_digit() => {
            let end = b.iter().take_while(|c| c.is_ascii_digit()).count();
            matches!(b.get(end), Some(b'.' | b')'))
        }
        _ => false,
    }
}

/// Render the whole model to one Typst document source.
#[must_use]
pub fn render(model: &DocModel) -> String {
    let mut out = String::new();
    out.push_str(
        "// Generated from the Mycelium corpus — a projection, never a parallel truth (ADR-003/G11).\n\
         // Compile with: typst compile doc.typ doc.pdf  (skipped gracefully when typst is absent).\n\
         #set document(title: \"Mycelium Documentation\")\n\
         #set page(numbering: \"1\", margin: (x: 2.2cm, y: 2.4cm))\n\
         #set text(font: \"New Computer Modern\", size: 10.5pt)\n\
         #set heading(numbering: \"1.1\")\n\
         // Print-legible code (§8.2): a light tinted box with a HAIRLINE border (never a filled dark\n\
         // panel), code ~0.82x body with tighter leading so lines fit without wrapping. Light-only.\n\
         #show raw.where(block: true): it => {\n\
         set text(size: 8.6pt)\n\
         set par(leading: 0.42em)\n\
         block(fill: rgb(\"#e4e9d9\"), stroke: 0.5pt + rgb(\"#cfd5c1\"), radius: 3pt, inset: (x: 8pt, y: 7pt), width: 100%, breakable: true, it)\n\
         }\n\n\
         #align(center)[#text(18pt)[*Mycelium Documentation*]]\n\
         #align(center)[_A projection of the cited corpus._]\n\n\
         #outline()\n\n",
    );
    for doc in &model.documents {
        render_doc(doc, &mut out);
    }
    out
}

fn render_doc(doc: &Node, out: &mut String) {
    out.push_str(&format!("// cid: {}\n", doc.id.as_str()));
    out.push_str(&format!(
        "= {}\n\n",
        escape(doc.title.as_deref().unwrap_or(&doc.anchor))
    ));
    for c in &doc.children {
        render_node(c, 2, out);
    }
    out.push('\n');
}

fn render_node(node: &Node, depth: usize, out: &mut String) {
    out.push_str(&format!("// cid: {}\n", node.id.as_str()));
    match &node.payload {
        Payload::Section => {
            let eq = "=".repeat(depth.clamp(2, 6));
            out.push_str(&format!(
                "{eq} {}\n\n",
                inline_typst(node.title.as_deref().unwrap_or(""))
            ));
            for c in &node.children {
                render_node(c, depth + 1, out);
            }
        }
        Payload::Prose { text } => {
            out.push_str(&render_prose_typst(text));
            out.push_str("\n\n");
        }
        Payload::Example { lang, source, .. } => {
            // Typst raw block; fence with backticks and the language tag. The closing fence must
            // start on its own line, so normalize to exactly one trailing newline in the body
            // (a source without a trailing newline would otherwise produce an invalid `…code````).
            let body = source.strip_suffix('\n').unwrap_or(source);
            out.push_str(&format!("```{lang}\n{body}\n```\n\n"));
        }
        Payload::ApiItem { signature, summary } => {
            let eq = "=".repeat(depth.clamp(2, 6));
            out.push_str(&format!(
                "{eq} `{}`\n\n",
                escape(signature.as_deref().unwrap_or(""))
            ));
            match summary {
                Some(s) => {
                    out.push_str(&escape(s));
                    out.push_str("\n\n");
                }
                None => out.push_str("_undocumented — no summary projected from source._\n\n"),
            }
            for c in &node.children {
                render_node(c, depth + 1, out);
            }
        }
        Payload::Undocumented { what } => {
            out.push_str(&format!("_undocumented: {}_\n\n", escape(what)));
        }
        Payload::Xref { target } => {
            out.push_str(&format!(
                "#link(\"{}\")[{}]\n\n",
                escape_str(&target.raw),
                escape(&target.raw)
            ));
        }
        Payload::Document { .. } | Payload::Index => {
            for c in &node.children {
                render_node(c, depth, out);
            }
        }
    }
}

/// Escape Typst markup metacharacters in body text. `pub(crate)` for white-box unit testing in
/// `src/tests/typst.rs`, not a downstream API.
///
/// `[`/`]` are escaped too: content flows into `#strong[…]`/`#emph[…]` **content arguments**, so an
/// unescaped `]` in the text would close the bracket early and break the `.typ`. (Inside `#raw("…")`
/// / `#link("…")` string arguments the delimiter is a quote, handled by [`escape_str`].)
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '\\' | '[' | ']' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Escape for a Typst **string literal** (used inside `#link("…")` and `#raw("…")`). A Typst string
/// cannot contain a raw newline (or other control char), so those are escaped too — otherwise a
/// projected inline-code span that captured a newline (e.g. an unbalanced backtick across the rows of
/// a projected pipe-table) would terminate the string early and break the whole `.typ`. Escaping
/// `\`, `"`, and the C0 controls makes ANY content a valid single-line string literal (G2: every
/// emitted `.typ` compiles).
fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
