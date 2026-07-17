//! White-box tests for [`crate::emit::html`] — extracted from the logic file (as-touched, CLAUDE.md
//! test layout rule) when the readability theme, syntax highlighting, sidebar/ToC, and pipe-table
//! rendering landed. Uses `pub(crate)` access to the table helpers.

use crate::corpus::{ingest, AnchorAlloc};
use crate::emit::html::{
    is_separator_cell, render, render_concat, render_table, split_row, template_hash,
};
use crate::ir::{DocModel, SourceKind};

fn model() -> DocModel {
    let mut a = AnchorAlloc::new();
    let src = "# Doc\n\nLead.\n\n## Sec\n\nBody with [a link](other.md#x).\n\n```myc-checked\nfn f() -> Binary{8} = 0b0\n```\n";
    let doc = ingest("docs/spec/doc.md", src, SourceKind::Spec, &mut a);
    DocModel::new(vec![doc])
}

#[test]
fn the_site_has_an_index_and_a_page_per_doc() {
    let m = model();
    let arts = render(&m, None);
    assert!(arts.files.contains_key("index.html"));
    assert_eq!(
        arts.files
            .keys()
            .filter(|k| k.starts_with("pages/"))
            .count(),
        1
    );
}

#[test]
fn every_node_id_is_embedded_for_parity() {
    let m = model();
    let html = render_concat(&m);
    for id in m.id_set() {
        assert!(html.contains(&id), "missing cid {id} in HTML");
    }
}

#[test]
fn the_template_is_one_and_pinned() {
    let m = model();
    let html = render_concat(&m);
    let th = template_hash();
    assert!(th.starts_with("blake3:"));
    assert!(html.contains("one template"));
}

#[test]
fn output_is_semantic_and_accessible() {
    let m = model();
    let html = render_concat(&m);
    // Exactly the `<main>` landmark the §4.1 legibility lint requires (bare tag; the focus target is
    // an inner `#content` wrapper).
    assert!(html.contains("<main>"));
    assert!(html.contains("id=\"content\""));
    assert!(html.contains("aria-label"));
    assert!(html.contains("lang=\"en\""));
    assert!(html.contains("class=\"language-"));
}

#[test]
fn the_page_carries_a_sidebar_search_toc_and_theme_toggle() {
    let m = model();
    let page = render(&m, None);
    let doc_html = page
        .files
        .iter()
        .find(|(k, _)| k.starts_with("pages/"))
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert!(doc_html.contains("class=\"sidebar\""));
    assert!(doc_html.contains("On this page"));
    assert!(doc_html.contains("corpus-search-box"));
    assert!(doc_html.contains("theme-toggle"));
    // The sidebar marks the current page.
    assert!(doc_html.contains("aria-current=\"page\""));
    // The self-contained theme is inlined (no external asset fetch): the CSS is inline, and there is
    // no external stylesheet link or remote script/asset src.
    assert!(doc_html.contains("<style>"));
    assert!(!doc_html.contains("<link "), "no external stylesheet");
    assert!(!doc_html.contains("src=\"http"), "no remote script/asset");
}

#[test]
fn the_emitted_css_ships_a_real_prefers_color_scheme_dark_rule() {
    // Real dark mode (not a capture-time-only override, CLAUDE.md docs-assets note): the emitted
    // stylesheet must honor the reader's OS preference by default AND let the persisted
    // `data-theme` toggle (crate::theme::THEME_TOGGLE_JS) win over it in both directions.
    let m = model();
    let html = render_concat(&m);
    assert!(
        html.contains("@media (prefers-color-scheme: dark)"),
        "no prefers-color-scheme media query in the emitted CSS"
    );
    // A genuine dark-palette value from crate::theme::READING_CSS's dark block (not the light
    // one) — proves the media query carries real color overrides, not an empty/no-op rule.
    assert!(
        html.contains("--paper:#14160f"),
        "prefers-color-scheme dark block is missing its dark palette value"
    );
    // The persisted-toggle override also resolves to the same dark palette in both directions.
    assert!(html.contains(":root[data-theme=\"dark\"]"));
    assert!(html.contains(":root[data-theme=\"light\"]"));
}

#[test]
fn myc_examples_are_lexically_highlighted_but_the_language_class_stays() {
    let m = model();
    let html = render_concat(&m);
    // The `language-` hook (the doc-lint requirement + highlight.js-compatible class) is preserved.
    assert!(html.contains("class=\"language-myc-checked\""));
    // The trusted L1 lexer coloured the tokens: `fn` keyword, `f(` function, `Binary` type.
    assert!(html.contains("class=\"tok-kw\""), "keyword highlighted");
    assert!(html.contains("class=\"tok-fn\""), "fn-position highlighted");
    assert!(html.contains("class=\"tok-type\""), "type highlighted");
}

#[test]
fn a_non_myc_example_is_not_highlighted_but_stays_escaped() {
    let mut a = AnchorAlloc::new();
    let src = "# D\n\nLead.\n\n## S\n\n```text\nfn not <highlighted> & raw\n```\n";
    let doc = ingest("docs/spec/d.md", src, SourceKind::Spec, &mut a);
    let html = render_concat(&DocModel::new(vec![doc]));
    assert!(html.contains("class=\"language-text\""));
    // No token SPANS are emitted for a non-myc block (the `.tok-*` classes still exist in the inlined
    // CSS, so we check for the span, not the bare class substring).
    assert!(
        !html.contains("<span class=\"tok-"),
        "non-myc must not be highlighted"
    );
    // Still escaped — never raw markup injection.
    assert!(html.contains("&lt;highlighted&gt;"));
}

#[test]
fn a_pipe_table_in_prose_renders_as_a_real_table() {
    let mut a = AnchorAlloc::new();
    let src =
        "# T\n\nLead.\n\n## Header\n\n| Field | Value |\n|-------|-------|\n| Status | Accepted |\n";
    let doc = ingest("docs/rfcs/RFC-0001.md", src, SourceKind::Rfc, &mut a);
    let html = render_concat(&DocModel::new(vec![doc]));
    assert!(html.contains("<table"), "table rendered");
    assert!(html.contains("table-wrap"), "wrapped for overflow scroll");
    assert!(html.contains("<th>Field</th>"));
    assert!(html.contains("<td>Status</td>"));
    assert!(html.contains("<td>Accepted</td>"));
}

#[test]
fn render_table_detects_well_formed_tables_and_rejects_prose() {
    // Well-formed: header + matching-width separator.
    let t = "| A | B |\n|---|:-:|\n| 1 | 2 |";
    let out = render_table(t, "blake3:x").expect("a well-formed table");
    assert!(out.contains("<th>A</th>") && out.contains("<td>1</td>"));
    // Not a table: a stray dashed line whose cell count differs from the header.
    assert!(render_table("Intro line\n---\nmore prose", "cid").is_none());
    // Not a table: only one line.
    assert!(render_table("| a | b |", "cid").is_none());
}

#[test]
fn table_row_and_separator_helpers_are_exact() {
    assert_eq!(split_row("| a | b | c |"), vec!["a", "b", "c"]);
    assert_eq!(split_row("x | y"), vec!["x", "y"]);
    assert!(is_separator_cell("---"));
    assert!(is_separator_cell(":-:"));
    assert!(is_separator_cell("--:"));
    assert!(!is_separator_cell("ab"));
    assert!(!is_separator_cell(""));
}

#[test]
fn inline_markdown_renders_as_html_spans_in_prose() {
    let mut a = AnchorAlloc::new();
    let src = "# D\n\nLead.\n\n## S\n\nSee **bold**, *em*, `snippet`, and [site](https://ex.io).\n";
    let doc = ingest("docs/spec/d.md", src, SourceKind::Spec, &mut a);
    let html = render_concat(&DocModel::new(vec![doc]));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<em>em</em>"));
    assert!(html.contains("<code class=\"inl\">snippet</code>"));
    assert!(html.contains("<a class=\"x\" href=\"https://ex.io\">site</a>"));
    // The literal markdown is gone from the prose.
    assert!(!html.contains("**bold**"));
}

#[test]
fn table_cells_render_inline_markdown() {
    let mut a = AnchorAlloc::new();
    let src = "# T\n\n## H\n\n| Field | Value |\n|-------|-------|\n| **Status** | `Accepted` |\n";
    let doc = ingest("docs/rfcs/RFC-1.md", src, SourceKind::Rfc, &mut a);
    let html = render_concat(&DocModel::new(vec![doc]));
    assert!(html.contains("<td><strong>Status</strong></td>"));
    assert!(html.contains("<td><code class=\"inl\">Accepted</code></td>"));
}

#[test]
fn a_heading_renders_inline_in_the_h_tag_but_plain_in_nav() {
    let mut a = AnchorAlloc::new();
    let src = "# D\n\n## The `swap` operation\n\nBody.\n";
    let arts = render(
        &DocModel::new(vec![ingest(
            "docs/spec/d.md",
            src,
            SourceKind::Spec,
            &mut a,
        )]),
        None,
    );
    let page = arts
        .files
        .iter()
        .find(|(k, _)| k.starts_with("pages/"))
        .map(|(_, v)| v.as_str())
        .unwrap();
    // The heading body carries inline code...
    assert!(page.contains("<code class=\"inl\">swap</code>"));
    // ...but the "on this page" ToC shows plain text (no nested tags, no literal backticks).
    assert!(page.contains("The swap operation</a>"));
    assert!(!page.contains("The `swap` operation"));
}

#[test]
fn inline_markdown_is_not_applied_inside_code_blocks() {
    let mut a = AnchorAlloc::new();
    let src = "# D\n\n## S\n\n```text\nliteral **stars** and `ticks`\n```\n";
    let doc = ingest("docs/spec/d.md", src, SourceKind::Spec, &mut a);
    let html = render_concat(&DocModel::new(vec![doc]));
    // Inside <pre><code>, `**`/backticks stay literal — never <strong>/<code class="inl">.
    assert!(html.contains("literal **stars** and `ticks`"));
    assert!(!html.contains("<strong>stars</strong>"));
}

#[test]
fn the_sidebar_is_a_collapsible_short_labeled_semantic_plus_logical_tree() {
    let mut a = AnchorAlloc::new();
    let rfc = ingest(
        "docs/rfcs/RFC-0002-Swap.md",
        "# RFC-0002 — Swap Certificate & Split Regime\n\nLead.\n\n## S\n\nBody.\n",
        SourceKind::Rfc,
        &mut a,
    );
    let adr = ingest(
        "docs/adr/ADR-010-Verified.md",
        "# ADR-010 — Verified Numerics Foundation\n\nLead.\n\n## S\n\nBody.\n",
        SourceKind::Adr,
        &mut a,
    );
    let m = DocModel::new(vec![rfc, adr]);
    let rfc_anchor = m.documents[0].anchor.clone();
    let adr_anchor = m.documents[1].anchor.clone();
    // Semantic spine: one topical chapter naming the RFC.
    let nav = vec![("Language Reference".to_owned(), vec![rfc_anchor.clone()])];

    let arts = render(&m, Some(&nav));
    let idx = arts.files.get("index.html").unwrap();

    // Collapsible native <details>/<summary> groups, with counts.
    assert!(idx.contains("<details"));
    assert!(idx.contains("<summary>"));
    assert!(idx.contains("class=\"count\""));
    // Both a SEMANTIC ("Topics") and a LOGICAL ("By type") section.
    assert!(idx.contains("<p class=\"nav-title\">Topics</p>"));
    assert!(idx.contains("<p class=\"nav-title\">By type</p>"));
    assert!(idx.contains("Language Reference"), "the semantic chapter");
    assert!(
        idx.contains("RFCs") && idx.contains("ADRs"),
        "by-type groups"
    );
    // Short label (not the full title), with the full title in the tooltip.
    assert!(
        idx.contains("RFC-0002 \u{b7} Swap Certificate"),
        "short label"
    );
    assert!(
        idx.contains("Split Regime"),
        "full title kept in the tooltip"
    );
    // Nothing dropped: every doc is reachable in the logical tree (the ADR is in no chapter).
    assert!(idx.contains(&format!("{rfc_anchor}.html")));
    assert!(idx.contains(&format!("{adr_anchor}.html")));

    // On the RFC's own page, the group(s) holding it default to open (semantic + its type group).
    let page = arts.files.get(&format!("pages/{rfc_anchor}.html")).unwrap();
    assert!(page.contains("<details open>"), "current group is expanded");
    assert!(page.contains("aria-current=\"page\""));
}

#[test]
fn an_undocumented_api_item_renders_a_visible_marker() {
    let mut a = AnchorAlloc::new();
    let doc = crate::apiref::project_nodule(
        "x.myc",
        "// nodule: x\nnodule x\nfn g() -> Binary{8} = 0b0\n",
        &mut a,
    );
    let m = DocModel::new(vec![doc]);
    let html = render_concat(&m);
    assert!(html.contains("undocumented"));
}
