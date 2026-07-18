//! White-box tests for [`crate::emit::typst`] — extracted from the logic file (as-touched, CLAUDE.md
//! test layout rule) when the print code-legibility pass landed. Uses `pub(crate)` access to `escape`.

use crate::corpus::{ingest, AnchorAlloc};
use crate::emit::typst::{escape, render};
use crate::ir::{DocModel, SourceKind};

fn model() -> DocModel {
    let mut a = AnchorAlloc::new();
    let src = "# Doc\n\nLead.\n\n## Sec\n\nBody text.\n\n```myc\nfn f() = 0\n```\n";
    DocModel::new(vec![ingest("d.md", src, SourceKind::Rfc, &mut a)])
}

#[test]
fn typst_has_a_preamble_and_outline() {
    let typ = render(&model());
    assert!(typ.contains("#set document"));
    assert!(typ.contains("#outline()"));
}

#[test]
fn headings_use_typst_equals_syntax() {
    let typ = render(&model());
    assert!(typ.contains("= Doc"));
    assert!(typ.contains("== Sec"));
}

#[test]
fn every_block_carries_its_cid() {
    let m = model();
    let typ = render(&m);
    for id in m.id_set() {
        assert!(typ.contains(&id), "missing cid {id}");
    }
}

#[test]
fn body_metacharacters_are_escaped() {
    assert_eq!(escape("a #b $c*"), "a \\#b \\$c\\*");
}

#[test]
fn inline_markdown_renders_as_typst_markup() {
    let mut a = AnchorAlloc::new();
    let src = "# D\n\n## S\n\nSee **bold**, *em*, `snippet`, and [site](https://ex.io).\n";
    let doc = ingest("docs/spec/d.md", src, SourceKind::Spec, &mut a);
    let typ = render(&DocModel::new(vec![doc]));
    assert!(typ.contains("#strong[bold]"));
    assert!(typ.contains("#emph[em]"));
    assert!(typ.contains("#raw(\"snippet\")"));
    assert!(typ.contains("#link(\"https://ex.io\")[site]"));
    // The literal markdown is gone from the Typst prose.
    assert!(!typ.contains("**bold**"));
}

#[test]
fn leading_block_markup_and_inline_calls_are_neutralized_so_typst_compiles() {
    // A paragraph whose (block-parser-joined) lines start with Typst block-markup triggers
    // `/ - + = N.`, plus inline `` `code`(x) `` (curried-call trap), a `]` (bracket-arg trap), and
    // an external link — the exact shapes that broke `typst compile`.
    let mut a = AnchorAlloc::new();
    let src = "# T\n\n## S\n\nlead line with a `code`(f64) call and a bracket ] and **bold**\n\
               / slash-led line\n- dash-led line\n+ plus-led line\n= equals-led line\n\
               1. ordered-led line\nsee [site](https://ex.io)\n";
    let doc = ingest("docs/spec/d.md", src, SourceKind::Spec, &mut a);
    let typ = render(&DocModel::new(vec![doc]));

    // Leading block markup is guarded with a zero-width `#h(0pt)` so it is literal text.
    for lead in ["#h(0pt)/", "#h(0pt)-", "#h(0pt)+", "#h(0pt)=", "#h(0pt)1."] {
        assert!(typ.contains(lead), "missing leading-markup guard {lead:?}");
    }
    // An inline call is followed by a zero-width break so a trailing `(` is not a curried call.
    assert!(
        typ.contains("#raw(\"code\")\u{200b}("),
        "inline call not separated from ("
    );
    // `]` in body text is escaped so it cannot close a `#strong[…]`/`#emph[…]` argument early.
    assert!(typ.contains("\\]"), "] not escaped");

    // Skip-graceful end-to-end compile: only when a `typst` binary is available (env override or
    // PATH). Absent → the structural asserts above are the gate (same posture as the rest of the gate).
    if let Some(bin) = typst_binary() {
        let dir = std::env::temp_dir().join(format!(
            "mycdoc-typst-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let typ_path = dir.join("doc.typ");
        std::fs::write(&typ_path, &typ).unwrap();
        let status = std::process::Command::new(&bin)
            .arg("compile")
            .arg(&typ_path)
            .arg(dir.join("doc.pdf"))
            .status()
            .expect("spawn typst");
        assert!(status.success(), "typst compile failed on the emitted .typ");
        std::fs::remove_dir_all(&dir).ok();
    }
}

/// A `typst` binary to compile with, if one is available: `MYC_TYPST_BIN`, else `typst` on `PATH`.
/// `None` → the compile step skips gracefully (structural asserts still run).
fn typst_binary() -> Option<String> {
    if let Ok(p) = std::env::var("MYC_TYPST_BIN") {
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    // Probe PATH: a version call that spawns successfully means `typst` is usable.
    std::process::Command::new("typst")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| "typst".to_owned())
}

#[test]
fn code_blocks_get_a_print_legible_show_rule() {
    // The print pass (§8.2): body ~10.5pt, comfortable margins, and a `raw.where(block:true)` show
    // rule that renders code smaller (~0.82x) with tighter leading in a hairline-bordered tinted box
    // (never a filled dark panel). Assert the structural markers are emitted.
    let typ = render(&model());
    assert!(typ.contains("size: 10.5pt"), "body scale");
    assert!(typ.contains("margin:"), "comfortable margins");
    assert!(
        typ.contains("raw.where(block: true)"),
        "code show rule present"
    );
    assert!(typ.contains("size: 8.6pt"), "code is set smaller than body");
    assert!(typ.contains("leading: 0.42em"), "code has tighter leading");
    // A hairline stroke and a light fill, not a dark panel.
    assert!(typ.contains("stroke: 0.5pt"), "hairline border");
    assert!(typ.contains("fill: rgb(\"#e4e9d9\")"), "light tinted box");
}
