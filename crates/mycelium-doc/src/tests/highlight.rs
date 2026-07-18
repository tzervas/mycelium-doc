//! White-box tests for [`crate::highlight`] — the lexical Mycelium syntax highlighter. Covers the
//! faithfulness property (stripped tags == escaped source), the token classes, the fn-position
//! heuristic, and every never-silent fallback (non-myc language, non-ASCII, lexer error).

use crate::emit::html_escape;
use crate::highlight::{highlight, is_myc_lang, strip_tags};

/// The load-bearing G2 property: the highlighted HTML, with its tags stripped, is **exactly** the
/// escaped source — tokens are wrapped, nothing is added, dropped, or fabricated.
fn assert_faithful(source: &str) {
    let html = highlight("myc", source).expect("myc source highlights");
    assert_eq!(
        strip_tags(&html),
        html_escape(source),
        "highlighted text must equal the escaped source (source: {source:?})"
    );
}

#[test]
fn highlighting_is_byte_faithful_to_the_escaped_source() {
    assert_faithful("fn f() -> Binary{8} = 0b0\n");
    assert_faithful("nodule x\nfn g(a: Binary{8}) = a < b  // trailing note\n");
    assert_faithful("let y = \"a string\" in y\n");
    assert_faithful("// a leading comment\nfn h() = 0t0\n");
}

#[test]
fn tokens_are_classified_into_the_design_system_buckets() {
    let html = highlight("myc", "fn f() -> Binary{8} = 0b0\n").unwrap();
    assert!(html.contains("<span class=\"tok-kw\">fn</span>"), "keyword");
    assert!(
        html.contains("<span class=\"tok-fn\">f</span>"),
        "fn-position"
    );
    assert!(
        html.contains("<span class=\"tok-type\">Binary</span>"),
        "type"
    );
    assert!(
        html.contains("<span class=\"tok-num\">0b0</span>"),
        "number"
    );
    assert!(html.contains("<span class=\"tok-op\">"), "operator");
}

#[test]
fn a_comment_is_captured_and_classified() {
    let html = highlight("myc", "fn f() = 0  // why\n").unwrap();
    assert!(html.contains("<span class=\"tok-com\">// why</span>"));
}

#[test]
fn a_plain_identifier_is_not_a_function() {
    // `y` is used as a value, not called — it must NOT be `tok-fn` (renders plain, default ink).
    let html = highlight("myc", "fn f() = y\n").unwrap();
    assert!(!html.contains("<span class=\"tok-fn\">y</span>"));
    // But the identifier text itself is still present (plain, escaped).
    assert!(strip_tags(&html).contains('y'));
}

#[test]
fn guarantee_strength_members_fold_into_the_keyword_bucket() {
    // Exact/Proven/Empirical/Declared are reserved keyword vocabulary in the 7-class palette.
    let html = highlight("myc", "let s = Declared in s\n").unwrap();
    assert!(html.contains("<span class=\"tok-kw\">Declared</span>"));
}

#[test]
fn a_non_myc_language_is_not_highlighted() {
    assert!(highlight("rust", "fn main() {}").is_none());
    assert!(highlight("text", "anything").is_none());
    assert!(highlight("ebnf", "program ::= x").is_none());
    assert!(is_myc_lang("myc"));
    assert!(is_myc_lang("myc-checked"));
    assert!(!is_myc_lang("rust"));
}

#[test]
fn a_non_ascii_source_falls_back_never_mis_offset() {
    // The UTF-16/scalar stopgap: a non-ASCII byte (here in a comment) yields the plain fallback.
    assert!(highlight("myc", "fn f() = y  // caf\u{00e9}\n").is_none());
}

#[test]
fn a_lexer_error_falls_back_to_plain() {
    // A lone backslash is an unrecognized ASCII character — the lexer errors, so we fall back to the
    // plain escaped source (never a fabricated span). ASCII-only, so this exercises the lexer-error
    // path specifically (not the non-ASCII stopgap above).
    assert!(highlight("myc", "fn f() = \\ x").is_none());
}
