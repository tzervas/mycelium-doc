//! White-box tests for [`crate::inline`] — the inline-markdown subset parser. Covers the core spans,
//! the "inline code first" rule, the `_`-not-intra-word rule (snake_case safety), link capture, and
//! the never-drop / never-panic behaviour on unbalanced markers.

use crate::inline::{is_external, parse, Span};

#[test]
fn parses_the_core_spans() {
    assert_eq!(
        parse("**bold**"),
        vec![Span::Strong(vec![Span::Text("bold")])]
    );
    assert_eq!(parse("*em*"), vec![Span::Em(vec![Span::Text("em")])]);
    assert_eq!(parse("_em_"), vec![Span::Em(vec![Span::Text("em")])]);
    assert_eq!(parse("`code`"), vec![Span::Code("code")]);
}

#[test]
fn inline_code_is_verbatim_and_wins_over_emphasis() {
    // The requirement: inline code FIRST, so `*`/`_` inside backticks are not emphasis.
    assert_eq!(parse("`*x* _y_`"), vec![Span::Code("*x* _y_")]);
}

#[test]
fn underscore_does_not_open_emphasis_intra_word() {
    // snake_case / file_index in prose must survive intact (the CommonMark flanking rule).
    assert_eq!(parse("file_index"), vec![Span::Text("file_index")]);
    assert_eq!(
        parse("a snake_case id"),
        vec![Span::Text("a snake_case id")]
    );
    // But a word-bounded `_em_` still works.
    assert_eq!(
        parse("an _emphasised_ word"),
        vec![
            Span::Text("an "),
            Span::Em(vec![Span::Text("emphasised")]),
            Span::Text(" word"),
        ]
    );
}

#[test]
fn a_link_captures_text_and_strips_a_title_suffix() {
    assert_eq!(
        parse("[docs](https://x.io)"),
        vec![Span::Link {
            text: vec![Span::Text("docs")],
            href: "https://x.io"
        }]
    );
    assert_eq!(
        parse("[a](b.md \"title\")"),
        vec![Span::Link {
            text: vec![Span::Text("a")],
            href: "b.md"
        }]
    );
}

#[test]
fn unbalanced_or_flanking_failing_markers_stay_literal_never_dropped() {
    assert_eq!(parse("a ** b"), vec![Span::Text("a ** b")]);
    assert_eq!(parse("`unclosed"), vec![Span::Text("`unclosed")]);
    // Opener immediately followed by a space does not open emphasis.
    assert_eq!(parse("* not em"), vec![Span::Text("* not em")]);
}

#[test]
fn mixed_prose_splits_into_text_and_spans_in_order() {
    assert_eq!(
        parse("see **bold** and `code` now"),
        vec![
            Span::Text("see "),
            Span::Strong(vec![Span::Text("bold")]),
            Span::Text(" and "),
            Span::Code("code"),
            Span::Text(" now"),
        ]
    );
}

#[test]
fn nesting_is_parsed_one_level_down() {
    // `**strong with `code`**` → strong wrapping text + code.
    assert_eq!(
        parse("**a `b` c**"),
        vec![Span::Strong(vec![
            Span::Text("a "),
            Span::Code("b"),
            Span::Text(" c"),
        ])]
    );
}

#[test]
fn emphasis_and_strong_nest_both_ways_without_stray_markers() {
    // strong-containing-em.
    assert_eq!(
        parse("**a *b* c**"),
        vec![Span::Strong(vec![
            Span::Text("a "),
            Span::Em(vec![Span::Text("b")]),
            Span::Text(" c"),
        ])]
    );
    // em-containing-strong: the single-`*` closer must SKIP the `**` runs (not close on one of them),
    // so no stray `**` is left behind.
    assert_eq!(
        parse("*a **b** c*"),
        vec![Span::Em(vec![
            Span::Text("a "),
            Span::Strong(vec![Span::Text("b")]),
            Span::Text(" c"),
        ])]
    );
}

#[test]
fn non_ascii_text_is_handled_without_panic() {
    // A multi-byte char between markers must not break slicing (UTF-8-safe advance).
    assert_eq!(
        parse("caf\u{00e9} **b**"),
        vec![
            Span::Text("caf\u{00e9} "),
            Span::Strong(vec![Span::Text("b")]),
        ]
    );
}

#[test]
fn link_text_with_balanced_brackets_is_matched_not_mis_cut() {
    // `[List[0]](url)` — the closing `]` is bracket-depth matched, so the link text keeps its inner
    // brackets instead of being cut at the wrong `]`.
    assert_eq!(
        parse("[List[0]](https://x.io)"),
        vec![Span::Link {
            text: vec![Span::Text("List[0]")],
            href: "https://x.io"
        }]
    );
    // Genuinely unbalanced bracket text finds no match → the leading `[` stays literal (no panic).
    let spans = parse("[a[b unbalanced");
    assert_eq!(spans, vec![Span::Text("[a[b unbalanced")]);
}

#[test]
fn is_external_classifies_hrefs() {
    assert!(is_external("https://x"));
    assert!(is_external("http://x"));
    assert!(is_external("mailto:a@b"));
    assert!(!is_external("../adr/ADR-001.md"));
    assert!(!is_external("#frag"));
}
