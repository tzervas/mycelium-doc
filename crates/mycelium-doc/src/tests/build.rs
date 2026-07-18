//! White-box tests for [`crate::build`] — extracted from the logic file (as-touched, CLAUDE.md test
//! layout rule) when `extra_md_files` was added. Uses `pub(crate)` access to `classify`,
//! `classify_target`, `normalize_join`, and `ResolveCtx`.

use crate::build::*;
use crate::ir::SourceKind;
use crate::ir::XrefResolution;

#[test]
fn normalize_join_resolves_dot_dot() {
    assert_eq!(
        normalize_join("docs/rfcs", "../adr/ADR-003.md"),
        "docs/adr/ADR-003.md"
    );
    assert_eq!(normalize_join("docs", "./Glossary.md"), "docs/Glossary.md");
    assert_eq!(normalize_join("docs/spec", "x.md"), "docs/spec/x.md");
}

#[test]
fn classify_maps_paths_to_families() {
    assert_eq!(classify("docs/rfcs/RFC-0001.md"), SourceKind::Rfc);
    assert_eq!(classify("docs/adr/ADR-010.md"), SourceKind::Adr);
    assert_eq!(classify("docs/notes/DN-06.md"), SourceKind::Note);
    assert_eq!(classify("docs/devlog/x.md"), SourceKind::Devlog);
    assert_eq!(classify("docs/spec/SPEC.md"), SourceKind::Spec);
    assert_eq!(classify("docs/Glossary.md"), SourceKind::Other);
    // A repo-root file outside `docs/` (e.g. CONTRIBUTING.md via `extra_md_files`) is `Other` too —
    // no substring match, same as any other unclassified corpus doc.
    assert_eq!(classify("CONTRIBUTING.md"), SourceKind::Other);
}

fn ctx_with(files: &[(&str, &str)], anchors: &[&str], corpus: &str) -> ResolveCtx {
    ResolveCtx {
        anchors: anchors.iter().map(|s| (*s).to_owned()).collect(),
        file_index: files
            .iter()
            .map(|(p, a)| ((*p).to_owned(), (*a).to_owned()))
            .collect(),
        corpus_rel: Some(corpus.to_owned()),
    }
}

#[test]
fn an_external_url_is_out_of_scope_not_dead() {
    let ctx = ctx_with(&[], &[], "docs");
    assert_eq!(
        classify_target("https://example.com", "d--x", "docs/a.md", &ctx),
        XrefResolution::ExternalUrl
    );
}

#[test]
fn a_resolving_internal_md_link_is_internal() {
    let ctx = ctx_with(
        &[("docs/rfcs/RFC-0013.md", "rfc-0013")],
        &["rfc-0013", "rfc-0013--levels"],
        "docs",
    );
    // file-level
    assert_eq!(
        classify_target("../rfcs/RFC-0013.md", "spec--x", "docs/spec/a.md", &ctx),
        XrefResolution::Internal {
            anchor: "rfc-0013".to_owned()
        }
    );
    // fragment-level
    assert_eq!(
        classify_target(
            "../rfcs/RFC-0013.md#levels",
            "spec--x",
            "docs/spec/a.md",
            &ctx
        ),
        XrefResolution::Internal {
            anchor: "rfc-0013--levels".to_owned()
        }
    );
}

#[test]
fn a_broken_internal_corpus_link_is_dead() {
    let ctx = ctx_with(&[], &[], "docs");
    match classify_target("../rfcs/RFC-9999.md", "spec--x", "docs/spec/a.md", &ctx) {
        XrefResolution::Dead { .. } => {}
        other => panic!("expected Dead, got {other:?}"),
    }
}

#[test]
fn a_link_outside_the_corpus_is_out_of_scope() {
    let ctx = ctx_with(&[], &[], "docs");
    // README at repo root — links.sh owns it, not the doc-IR.
    assert_eq!(
        classify_target("../../README.md", "spec--x", "docs/spec/a.md", &ctx),
        XrefResolution::OutOfScope
    );
    // a non-markdown target
    assert_eq!(
        classify_target("../../scripts/lib.sh", "spec--x", "docs/spec/a.md", &ctx),
        XrefResolution::OutOfScope
    );
}

#[test]
fn a_missing_fragment_falls_back_to_the_document_top() {
    let ctx = ctx_with(
        &[("docs/x.md", "x")],
        &["x"], // no x--nope anchor
        "docs",
    );
    assert_eq!(
        classify_target("x.md#nope", "y--a", "docs/y.md", &ctx),
        XrefResolution::Internal {
            anchor: "x".to_owned()
        }
    );
}

#[test]
fn extra_md_files_are_ingested_and_their_xrefs_resolve() {
    // A hermetic tiny tree: docs/rfcs/RFC-0001.md is the corpus; CONTRIBUTING.md sits at the repo
    // root and links to it — proving extra_md_files goes through the SAME resolve pipeline as the
    // corpus walk (not a silently-unresolved bolt-on).
    let root = std::env::temp_dir().join(format!(
        "mycdoc-extra-md-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("docs/rfcs")).unwrap();
    std::fs::write(
        root.join("docs/rfcs/RFC-0001-Thing.md"),
        "# RFC-0001 — Thing\n\nAbstract.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("CONTRIBUTING.md"),
        "# Contributing\n\nSee [RFC-0001](docs/rfcs/RFC-0001-Thing.md).\n",
    )
    .unwrap();

    let mut input = BuildInput::conventional(&root);
    input.extra_md_files = vec![root.join("CONTRIBUTING.md")];
    let model = build(&input).expect("build succeeds");

    let contributing = model
        .documents
        .iter()
        .find(|d| d.provenance.source == "CONTRIBUTING.md")
        .expect("CONTRIBUTING.md was ingested");
    assert_eq!(contributing.title.as_deref(), Some("Contributing"));

    let mut found_internal = false;
    contributing.walk(&mut |n| {
        if let crate::ir::Payload::Xref { target } = &n.payload {
            if matches!(target.resolution, XrefResolution::Internal { .. }) {
                found_internal = true;
            }
        }
    });
    assert!(
        found_internal,
        "CONTRIBUTING.md's link to the RFC should resolve internally"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// A hermetic temp tree standing in for the repo layout, so `research_root`/`extra_md_files`
/// tests don't depend on the real `research/`/`CONTRIBUTING.md` (which move independently of this
/// test suite).
fn temp_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mycdoc-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn conventional_ingests_research_root() {
    // The doc-ingestion gap this closes: `BuildInput::conventional` used to walk only `docs/` +
    // `docs/spec/schemas` + `examples/`/`lib/std` — never `research/` — so a manifest cluster
    // globbing `research/*-RECORD.md` (tools/docgen/notebooklm/research.json) always resolved to
    // zero pages. `conventional` now sets `research_root`, and `build` walks it the same
    // skip-graceful, sorted flat-markdown way as `corpus_root`.
    let root = temp_root("research-root");
    std::fs::create_dir_all(root.join("research")).unwrap();
    std::fs::write(
        root.join("research/01-example-RECORD.md"),
        "# 01 — Example Record\n\nFindings.\n",
    )
    .unwrap();

    let input = BuildInput::conventional(&root);
    assert_eq!(input.research_root, Some(root.join("research")));
    let model = build(&input).expect("build succeeds");

    let record = model
        .documents
        .iter()
        .find(|d| d.provenance.source == "research/01-example-RECORD.md")
        .expect("research/*-RECORD.md was ingested");
    assert_eq!(record.title.as_deref(), Some("01 — Example Record"));
    // Not under docs/rfcs//adr//notes//devlog//spec/ — falls to Other, same as any other
    // unclassified corpus doc (CONTRIBUTING.md, Glossary.md).
    assert_eq!(classify(&record.provenance.source), SourceKind::Other);

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn conventional_leaves_research_root_absent_gracefully() {
    // A repo tree without a `research/` directory (e.g. a hermetic test fixture) must not error —
    // the same skip-graceful posture `example_roots` already has.
    let root = temp_root("no-research");
    std::fs::create_dir_all(root.join("docs")).unwrap();
    let input = BuildInput::conventional(&root);
    let model = build(&input).expect("build succeeds without a research/ directory");
    assert!(model
        .documents
        .iter()
        .all(|d| !d.provenance.source.starts_with("research/")));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn conventional_ingests_contributing_md_by_default() {
    // `docs/book-manifest.json`'s Contributing chapter names `CONTRIBUTING.md`, and
    // `myc-doc build --manifest` (unlike the `book` subcommand) built its model straight from
    // `BuildInput::conventional` with no override — so CONTRIBUTING.md never resolved on that path.
    // `conventional` now seeds `extra_md_files` with it directly (the `book` CLI arm no longer needs
    // its own override — same field, same pipeline, set once).
    let root = temp_root("contributing-default");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("CONTRIBUTING.md"),
        "# Contributing\n\nGuidelines.\n",
    )
    .unwrap();

    let input = BuildInput::conventional(&root);
    assert_eq!(input.extra_md_files, vec![root.join("CONTRIBUTING.md")]);
    let model = build(&input).expect("build succeeds");

    let contributing = model
        .documents
        .iter()
        .find(|d| d.provenance.source == "CONTRIBUTING.md")
        .expect("CONTRIBUTING.md was ingested by default");
    assert_eq!(contributing.title.as_deref(), Some("Contributing"));

    std::fs::remove_dir_all(&root).ok();
}
