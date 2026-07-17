//! White-box tests for [`crate::label`] — the concise sidebar label derivation. Deterministic,
//! never-blank, ID-prefixed for records, module name for stdlib specs, category-prefix stripped for
//! the rest.

use crate::ir::{Node, Payload, Provenance, SourceKind};
use crate::label::short_label;

fn doc(anchor: &str, title: &str, source: &str) -> Node {
    Node::new(
        anchor,
        Some(title.to_owned()),
        None,
        Provenance {
            source: source.to_owned(),
            line: 1,
        },
        Payload::Document {
            source_kind: SourceKind::Other,
        },
        vec![],
    )
}

#[test]
fn idd_docs_get_id_plus_short_title() {
    let rfc = doc(
        "rfc-0002-swap-certificate-and-split-regime",
        "RFC-0002 — Swap Certificate & Split Regime",
        "docs/rfcs/RFC-0002-Swap-Certificate-and-Split-Regime.md",
    );
    let l = short_label(&rfc);
    assert!(
        l.starts_with("RFC-0002 \u{b7} Swap Certificate"),
        "got {l:?}"
    );

    // A `:`-separated ADR title.
    let adr = doc(
        "adr-012-layered-lexicon",
        "ADR-012: Layered Lexicon and Fungal Runtime Model",
        "docs/adr/ADR-012.md",
    );
    assert!(
        short_label(&adr).starts_with("ADR-012 \u{b7} Layered Lexicon"),
        "got {:?}",
        short_label(&adr)
    );

    // A `Design Note DN-…` prefix, cut at `(`.
    let dn = doc(
        "dn-114-validated-narrative-generation",
        "Design Note DN-114 — Validated Narrative Generation (reveal + E1/E2)",
        "docs/notes/DN-114.md",
    );
    assert_eq!(
        short_label(&dn),
        "DN-114 \u{b7} Validated Narrative Generation"
    );
}

#[test]
fn stdlib_specs_get_the_module_name() {
    let vsa = doc(
        "vsa",
        "Spec — `std.vsa` (hdc) (hypervector algebra…)",
        "docs/spec/stdlib/vsa.md",
    );
    assert_eq!(short_label(&vsa), "std.vsa");
    let cmp = doc(
        "cmp",
        "Spec — `std.cmp` / `convert` (ordering/equality…)",
        "docs/spec/stdlib/cmp.md",
    );
    assert_eq!(short_label(&cmp), "std.cmp");
}

#[test]
fn generic_docs_strip_the_category_prefix_and_cap() {
    // A spec: the `Spec (Proposed) —` prefix is stripped, then cut at `(`.
    let spec = doc(
        "lint-and-autofix-contract",
        "Spec (Proposed) — lint + auto-fix contract (actionable diagnostics)",
        "docs/spec/Lint-and-Autofix-Contract.md",
    );
    assert_eq!(short_label(&spec), "lint + auto-fix contract");

    // A guide/other `Mycelium — X` title strips to the real title (never collapses to "Mycelium").
    let gloss = doc(
        "glossary",
        "Mycelium — Glossary & Term Index",
        "docs/Glossary.md",
    );
    assert_eq!(short_label(&gloss), "Glossary & Term Index");
}

#[test]
fn short_label_is_never_blank_and_falls_back_to_the_anchor() {
    // No title → the anchor.
    let n = Node::new(
        "some-page",
        None,
        None,
        Provenance {
            source: "docs/x.md".to_owned(),
            line: 1,
        },
        Payload::Document {
            source_kind: SourceKind::Other,
        },
        vec![],
    );
    assert_eq!(short_label(&n), "some-page");
    assert!(!short_label(&n).is_empty());
}
