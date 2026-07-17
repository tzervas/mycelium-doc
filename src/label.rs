//! Concise **navigation labels** for the sidebar tree. A document's full H1 title is often long
//! ("RFC-0002 — Swap Certificate & Split Regime", "Spec — `std.vsa` (hdc) (hypervector…)") — listing
//! those verbatim makes the nav wrap to several lines and become an unreadable wall. [`short_label`]
//! derives a short, deterministic label; the caller keeps the **full title in a `title=""` tooltip**,
//! so nothing is lost.
//!
//! Deterministic + never-silent (G2): the rules are pure functions of the node's title / anchor /
//! source; if a title cannot be shortened cleanly it falls back to a hard char-cap with an ellipsis —
//! **never blank**.

use crate::ir::Node;

/// A concise navigation label for `node` (see the module rules). Never empty. Inline markdown is
/// stripped up front ([`crate::inline::to_plain`]), so word-capping can never split a `` `code` ``
/// span and leave a dangling backtick in the label.
///
/// - **ID'd** (`RFC-NNNN`/`ADR-NNN`/`DN-NN`): `"<ID> · <short title>"` — the title with the ID (and a
///   `Design Note ` prefix) stripped, cut at the first of `—`/`(`/`:`, word-capped ~30.
/// - **Stdlib spec** (`docs/spec/stdlib/…`): the module name (`std.vsa`).
/// - **Everything else**: the title (a leading `Spec … —` stripped), cut at the first of `—`/`(`/`:`,
///   word-capped ~32.
#[must_use]
pub fn short_label(node: &Node) -> String {
    let raw = node.title.as_deref().unwrap_or(&node.anchor);
    let plain = crate::inline::to_plain(raw);
    let title = plain.trim();
    if title.is_empty() {
        return node.anchor.clone();
    }

    // 1. ID'd records.
    if let Some(id) = doc_id(&node.anchor) {
        let short = shorten(after_id(title, &id), 30);
        return if short.is_empty() {
            id
        } else {
            format!("{id} \u{b7} {short}")
        };
    }

    // 2. Stdlib module specs.
    if node.provenance.source.contains("/stdlib/") {
        if let Some(m) = std_module(title) {
            return m;
        }
    }

    // 3. Everything else: strip a leading category prefix (`Spec … —`, `Mycelium — …`, …) so the
    //    real title shows — the corpus's non-ID docs are overwhelmingly `<Prefix> — <Real title>`, and
    //    cutting *at* the em-dash would collapse them all to the shared prefix (e.g. "Mycelium").
    let base = after_em_dash(title).unwrap_or(title);
    let short = shorten(base, 32);
    if short.is_empty() {
        hard_cap(title, 30)
    } else {
        short
    }
}

/// The display ID (`RFC-0002`/`ADR-010`/`DN-01`) parsed from a doc anchor slug, if it is an ID'd doc.
fn doc_id(anchor: &str) -> Option<String> {
    for (pfx, up) in [("rfc-", "RFC-"), ("adr-", "ADR-"), ("dn-", "DN-")] {
        if let Some(rest) = anchor.strip_prefix(pfx) {
            let num: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !num.is_empty() {
                return Some(format!("{up}{num}"));
            }
        }
    }
    None
}

/// The part of `title` after the (case-sensitive) `id` occurrence, with leading separators/space
/// trimmed. Falls back to the whole title if the id is not found in it.
fn after_id<'a>(title: &'a str, id: &str) -> &'a str {
    match title.find(id) {
        Some(pos) => title[pos + id.len()..].trim_start_matches(|c: char| !c.is_alphanumeric()),
        None => title,
    }
}

/// The part of `title` after the first em-dash (`—`), trimmed — for stripping a `Spec … —` prefix.
fn after_em_dash(title: &str) -> Option<&str> {
    title.split_once('\u{2014}').map(|(_, rest)| rest.trim())
}

/// The `std.<module>` token in a stdlib spec title (`Spec — \`std.vsa\` (…)` → `std.vsa`).
fn std_module(title: &str) -> Option<String> {
    let start = title.find("std.")?;
    let tail = &title[start..];
    let end = tail
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(tail.len());
    let m = tail[..end].trim_end_matches('.');
    (m.len() > "std".len()).then(|| m.to_owned())
}

/// Cut `s` at the first of `—`/`(`/`:`, then word-cap to ~`cap` chars (with an ellipsis when it must
/// truncate mid-title). Deterministic; empty only if `s` is empty/whitespace.
fn shorten(s: &str, cap: usize) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let mut cut = s.len();
    for pat in ["\u{2014}", "(", ":"] {
        if let Some(p) = s.find(pat) {
            cut = cut.min(p);
        }
    }
    let head = s[..cut].trim();
    if head.chars().count() <= cap {
        return head.to_owned();
    }
    let mut out = String::new();
    for w in head.split_whitespace() {
        let add = w.chars().count() + usize::from(!out.is_empty());
        if out.chars().count() + add > cap {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    if out.is_empty() {
        out = head.chars().take(cap).collect();
    }
    out.push('\u{2026}');
    out
}

/// Hard fallback: the first `cap` chars of `s` plus an ellipsis (never blank unless `s` is).
fn hard_cap(s: &str, cap: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= cap {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(cap).collect();
    out.push('\u{2026}');
    out
}
