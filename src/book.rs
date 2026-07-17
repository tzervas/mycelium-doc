//! The **BOOK** output — M-363's output (b), *"the full language book"* (spec §4 `gen-book`): a
//! curated, linear, chaptered reading order over the doc-IR, with per-page prev/next navigation and a
//! client-side search index. This is a **fifth renderer alongside HTML/Typst/JSON**
//! ([`crate::emit`]) — it composes the *existing* honest per-page HTML projection into a book; it
//! does **not** re-author content (spec §4: "projection, not authorship").
//!
//! ## Curated ordering, not a parallel truth
//! A book needs a reading order the flat, alphabetical-by-`SourceKind` corpus index doesn't have
//! (the ratified spec §4 calls `gen-book` **"projection + light interpretation"** — sequencing is the
//! interpretive part; the *content* on each page is still pure projection). The order is a small,
//! committed manifest ([`docs/book-manifest.json`](../../../../docs/book-manifest.json)), **not**
//! hand-edited generated output: each chapter lists explicit `sources` (curated order) and/or
//! `globs` (drift-proof — a new stdlib/RFC/ADR/DN file is picked up automatically, the same
//! `tools/docgen/code_index.py` discipline). A manifest entry that resolves to **no** ingested
//! document is a **build error** (never a silently-dropped chapter, never a dead link — G2).
//!
//! ## Composing, not re-rendering
//! Each book page's body is the **same** `<article>` HTML [`crate::emit::html`] already produced for
//! the corpus site (byte-identical `data-cid` attributes and all) — this module renders a *scoped*
//! [`DocModel`] (exactly the book's pages) through [`crate::emit::html::render`] and re-wraps the
//! extracted article in a book-specific shell (chapter breadcrumb, prev/next, a ToC/search
//! sidebar link). Two non-corpus sources are honestly, explicitly composed in too:
//! - `CONTRIBUTING.md` (repo root, outside `docs/`) rides in via [`crate::build::BuildInput::extra_md_files`]
//!   so it is a genuine ingested [`Node`], not a special case here.
//! - `docs/spec/grammar/mycelium.ebnf` is not markdown, so [`crate::build::build`] never walks it; this
//!   module synthesizes a single [`Payload::Document`] node wrapping its content **verbatim** as an
//!   unchecked [`Payload::Example`] (grounded — the exact file bytes — never invented prose).
//!
//! No new dependency: manifest parsing reuses `serde`/`serde_json` (already vetted, KC-3); the search
//! index and its client-side filter are hand-rolled JSON + vanilla JS, the same "no heavy dep"
//! posture as the rest of this crate.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::emit::{html_escape, Artifacts};
use crate::inline;
use crate::ir::{DocModel, Level, Node, Payload, Provenance, SourceKind};
use crate::theme;

/// An HTML nav label: inline markdown stripped ([`inline::to_plain`]) then HTML-escaped — so a
/// backtick-bearing title (e.g. ``"DN-102 · The `?` Try-Operator"``) reads cleanly in the book nav
/// instead of leaking literal markdown (mirrors `emit::html`'s `inline_text`).
fn nav_label(text: &str) -> String {
    html_escape(&inline::to_plain(text))
}

/// The repo-relative default location of the committed chapter manifest.
pub const DEFAULT_MANIFEST_PATH: &str = "docs/book-manifest.json";

/// A never-silent book-build error (a broken manifest entry, a bad manifest, an anchor collision) —
/// surfaced with enough detail to fix it, never a silently-dropped chapter (G2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookError(pub String);

impl fmt::Display for BookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for BookError {}

/// The committed chapter manifest (`docs/book-manifest.json`) — curated order, drift-proof globs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookManifest {
    /// The book's title (the ToC page `<h1>`).
    pub title: String,
    /// A short, hand-authored preface (the one piece of new prose this module authors — spec
    /// "minimal new authoring"; everything else is projected).
    pub preface: String,
    /// Chapters, in reading order.
    pub chapters: Vec<ChapterSpec>,
}

/// One chapter: an ordered list of explicit sources, optionally extended by drift-proof globs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSpec {
    /// The chapter title.
    pub title: String,
    /// Explicit repo-relative source paths, in curated reading order.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Glob patterns (a single `*` wildcard per pattern — a hand-rolled subset, not a full glob
    /// engine, the same "honestly a subset" discipline as [`crate::corpus`]'s markdown parser).
    /// Matches are resolved against every ingested document's source path, sorted, and appended
    /// after `sources` — so a new file under the globbed directory is picked up automatically.
    #[serde(default)]
    pub globs: Vec<String>,
    /// Source paths to exclude from a `globs` match (e.g. a module `README.md`/template file).
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Load the committed manifest from `<repo_root>/docs/book-manifest.json`.
///
/// # Errors
/// A missing or unparseable manifest is a `BookError`, never a silent empty book.
pub fn load_manifest(repo_root: &Path) -> Result<BookManifest, BookError> {
    load_manifest_from(&repo_root.join(DEFAULT_MANIFEST_PATH))
}

/// Load a manifest from an **explicit path** — the entry the `--manifest` scoped-emission path uses
/// (a per-cluster manifest, the same `{title, chapters:[{sources, globs, exclude}]}` shape as the
/// committed book manifest). [`load_manifest`] is the convenience wrapper for the default location.
///
/// # Errors
/// A missing or unparseable manifest is a `BookError`, never a silent empty subset (G2).
pub fn load_manifest_from(path: &Path) -> Result<BookManifest, BookError> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| BookError(format!("reading {}: {e}", path.display())))?;
    serde_json::from_str(&src).map_err(|e| BookError(format!("parsing {}: {e}", path.display())))
}

/// A hand-rolled glob-**subset** matcher — the "honestly a subset, named as one" convention — over
/// three metacharacters: `*` (any run, including empty), `?` (any one char), and `[…]` **character
/// classes** with ranges (`[0-9]`, `[a-z]`), literal sets (`[abc]`), and negation (`[!…]`/`[^…]`).
/// A literal `]` may lead the class (`[]…]`); an unterminated `[` matches a literal `[`. No `**`, no
/// path-segment semantics (`*` spans `/`) — enough for the committed manifests' bracket globs
/// (`docs/notes/DN-[5-9][0-9]-*.md`, `DN-1[0-9][0-9]-*.md`), which the old single-`*` matcher could
/// not express (it resolved those clusters to zero pages — a silent Markdown fallback, now fixed).
///
/// `pub(crate)` for white-box unit testing in `src/tests/book.rs`, not a downstream API.
pub(crate) fn glob_match(pattern: &str, candidate: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let c: Vec<char> = candidate.chars().collect();
    glob_match_at(&p, 0, &c, 0)
}

/// Match `pattern[pi..]` against `candidate[ci..]` (recursive; `*` backtracks).
fn glob_match_at(p: &[char], mut pi: usize, c: &[char], mut ci: usize) -> bool {
    while pi < p.len() {
        match p[pi] {
            '*' => {
                while pi < p.len() && p[pi] == '*' {
                    pi += 1; // collapse consecutive `*`
                }
                if pi == p.len() {
                    return true; // trailing `*` matches the rest
                }
                return (ci..=c.len()).any(|k| glob_match_at(p, pi, c, k));
            }
            '?' => {
                if ci >= c.len() {
                    return false;
                }
                pi += 1;
                ci += 1;
            }
            '[' => match match_class(p, pi, c.get(ci).copied()) {
                Some((matched, next_pi)) => {
                    if !matched {
                        return false;
                    }
                    pi = next_pi;
                    ci += 1;
                }
                None => {
                    // Unterminated `[` → literal `[`.
                    if c.get(ci) != Some(&'[') {
                        return false;
                    }
                    pi += 1;
                    ci += 1;
                }
            },
            ch => {
                if c.get(ci) != Some(&ch) {
                    return false;
                }
                pi += 1;
                ci += 1;
            }
        }
    }
    ci == c.len()
}

/// Match one candidate char `ch` against the character class starting at `p[pi] == '['`. Returns
/// `Some((matched, next_pi))` where `next_pi` is just past the closing `]`, or `None` if the class is
/// unterminated (no closing `]`) — then the caller treats `[` as a literal. `ch == None` (candidate
/// exhausted) never matches a class.
fn match_class(p: &[char], pi: usize, ch: Option<char>) -> Option<(bool, usize)> {
    let mut j = pi + 1;
    let negate = matches!(p.get(j), Some('!') | Some('^'));
    if negate {
        j += 1;
    }
    let body_start = j;
    // Find the closing `]` (a `]` in the first body position is a literal member).
    let mut end = None;
    let mut k = j;
    while k < p.len() {
        if p[k] == ']' && k != body_start {
            end = Some(k);
            break;
        }
        k += 1;
    }
    let end = end?;
    let class = &p[body_start..end];
    let Some(ch) = ch else {
        return Some((false, end + 1)); // no candidate char: a class (it consumes one) cannot match
    };
    let mut matched = false;
    let mut m = 0;
    while m < class.len() {
        if m + 2 < class.len() && class[m + 1] == '-' {
            if class[m] <= ch && ch <= class[m + 2] {
                matched = true;
            }
            m += 3;
        } else {
            if ch == class[m] {
                matched = true;
            }
            m += 1;
        }
    }
    Some((matched ^ negate, end + 1))
}

/// Synthesize a single [`Payload::Document`] node wrapping a non-markdown file **verbatim** as an
/// unchecked example — grounded (the file's exact bytes), never invented. Used for the one
/// non-`.md` book source (`docs/spec/grammar/mycelium.ebnf`); `checked: false` is honest (it is a
/// grammar fragment, not a `.myc` program — the checked-examples lint only ever applies to real
/// nodule source, §4.1 #4).
fn synth_verbatim_node(anchor: &str, title: &str, path: &str, lang: &str, src: &str) -> Node {
    let prov = Provenance {
        source: path.to_owned(),
        line: 1,
    };
    let body = Node::new(
        format!("{anchor}--source"),
        None,
        Some(Level::Detailed),
        prov.clone(),
        Payload::Example {
            lang: lang.to_owned(),
            source: src.to_owned(),
            checked: false,
        },
        vec![],
    );
    Node::new(
        anchor.to_owned(),
        Some(title.to_owned()),
        None,
        prov,
        Payload::Document {
            source_kind: SourceKind::Spec,
        },
        vec![body],
    )
}

/// One resolved book page: which chapter it belongs to, and the doc-IR node it projects.
struct Page {
    chapter_idx: usize,
    node: Node,
}

/// Resolve every chapter's `sources`/`globs` against the model, synthesizing the one honest
/// exception (the grammar EBNF). Never-silent: an entry that resolves to nothing is a `BookError`.
fn resolve_pages(
    model: &DocModel,
    manifest: &BookManifest,
    repo_root: &Path,
) -> Result<Vec<Page>, BookError> {
    let by_source: BTreeMap<&str, &Node> = model
        .documents
        .iter()
        .map(|d| (d.provenance.source.as_str(), d))
        .collect();

    let mut pages = Vec::new();
    let mut seen_anchors: BTreeSet<String> = BTreeSet::new();
    let mut seen_sources: BTreeSet<String> = BTreeSet::new();

    for (chapter_idx, chapter) in manifest.chapters.iter().enumerate() {
        let mut ordered_paths: Vec<String> = chapter.sources.clone();
        if !chapter.globs.is_empty() {
            let mut matched: Vec<String> = model
                .documents
                .iter()
                .map(|d| d.provenance.source.clone())
                .filter(|p| chapter.globs.iter().any(|g| glob_match(g, p)))
                .filter(|p| !chapter.exclude.contains(p))
                .collect();
            matched.sort();
            ordered_paths.extend(matched);
        }
        if ordered_paths.is_empty() {
            return Err(BookError(format!(
                "chapter '{}' resolves to zero pages (empty sources/globs, or every glob match was \
                 excluded) — a chapter with no content is a broken book, not a silent skip",
                chapter.title
            )));
        }
        for path in ordered_paths {
            if !seen_sources.insert(path.clone()) {
                return Err(BookError(format!(
                    "'{path}' appears in more than one book chapter — a page must have exactly one \
                     place in the reading order"
                )));
            }
            let node = if let Some(&n) = by_source.get(path.as_str()) {
                n.clone()
            } else if path.ends_with(".ebnf") {
                let full = repo_root.join(&path);
                let src = std::fs::read_to_string(&full).map_err(|e| {
                    BookError(format!(
                        "chapter '{}': cannot read grammar source {path}: {e}",
                        chapter.title
                    ))
                })?;
                synth_verbatim_node(
                    "book-grammar-ebnf",
                    "Mycelium Grammar (EBNF)",
                    &path,
                    "ebnf",
                    &src,
                )
            } else {
                return Err(BookError(format!(
                    "chapter '{}' references '{path}' but no such document was ingested — fix \
                     docs/book-manifest.json or the source path (never a silently-dropped chapter)",
                    chapter.title
                )));
            };
            if !seen_anchors.insert(node.anchor.clone()) {
                return Err(BookError(format!(
                    "duplicate book page anchor '{}' (from '{path}') — an anchor collision would \
                     silently merge two distinct pages",
                    node.anchor
                )));
            }
            pages.push(Page { chapter_idx, node });
        }
    }
    Ok(pages)
}

/// Extract the `<article>...</article>` body from a full rendered page — reusing the *exact* honest
/// projection [`crate::emit::html::render`] already produced (same `data-cid`s), never re-deriving
/// content (spec §4: "projection, not authorship").
fn extract_article(page_html: &str) -> &str {
    let start = page_html.find("<article").unwrap_or(0);
    let end = page_html
        .rfind("</article>")
        .map_or(page_html.len(), |i| i + "</article>".len());
    &page_html[start..end]
}

/// One curated book page: a sticky header (with the theme toggle), a persistent **chapter-tree
/// sidebar** + reading `<main>` (the shared [`crate::theme`] layout, §5 — the same comfortable visual
/// language as the corpus site), and the book's own prev/next chrome inside `main`. `sidebar` is the
/// chapter tree; `breadcrumb`/pager live in `main`.
fn book_shell(book_title: &str, page_title: &str, sidebar: &str, main: &str) -> String {
    book_document(book_title, page_title, "../", sidebar, main)
}

/// The shared document skeleton for every book page (chapter page, ToC landing, search) — one
/// template, the self-contained offline theme, no-flash init, and the theme toggle. `index_prefix`
/// is `"../"` from a `book/pages/` page and `""` from `book/index.html`/`book/search.html`.
fn book_document(
    book_title: &str,
    page_title: &str,
    index_prefix: &str,
    sidebar: &str,
    main: &str,
) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <title>{page_title} — {book_title}</title>\n\
         <style>{css}</style>\n{head_init}\n\
         </head>\n<body>\n{skip}\n\
         <header class=\"site-header\"><div class=\"bar\">\
         <p class=\"site-title\">{book_title}</p>\
         <p class=\"tagline\">A curated linear composition of the honest corpus projection — never a \
         parallel truth (ADR-003/G11). <a href=\"{index_prefix}search.html\">Search the book</a></p>\
         {toggle}</div></header>\n\
         <div class=\"layout no-toc\">\n{sidebar}\n\
         <main id=\"content\">\n{main}\n</main>\n</div>\n\
         <footer>Generated by <code>myc-doc book</code> — every page composes the same \
         content-addressed article the corpus site renders (dual-projection parity by \
         construction). Undocumented items are flagged, never invented (G2).</footer>\n\
         {toggle_js}\n\
         </body>\n</html>\n",
        book_title = html_escape(book_title),
        page_title = nav_label(page_title),
        css = theme::READING_CSS,
        head_init = theme::HEAD_THEME_INIT,
        skip = theme::SKIP_LINK,
        toggle = theme::THEME_TOGGLE_BUTTON,
        toggle_js = theme::THEME_TOGGLE_JS,
    )
}

/// The persistent chapter-tree sidebar: each chapter is a **collapsible `<details>`** group of its
/// pages (short-labeled, full title in a `title=""` tooltip), plus a Table-of-contents / Search link
/// pair at the top. The chapter holding the current page defaults to `open` (others collapsed);
/// the current page keeps its `aria-current` highlight. `page_prefix` reaches a chapter page
/// (`""` from a `book/pages/` page, `"pages/"` from `book/index.html`); `index_prefix` reaches the
/// ToC/search (`"../"` from a page, `""` from the index).
fn book_sidebar(
    manifest: &BookManifest,
    pages: &[Page],
    page_prefix: &str,
    index_prefix: &str,
    current: Option<&str>,
) -> String {
    let mut nav = String::from("<nav class=\"sidebar\" aria-label=\"Book navigation\">\n");
    nav.push_str(&format!(
        "<ul>\n  <li><a href=\"{ip}index.html\">Table of contents</a></li>\n\
         \x20 <li><a href=\"{ip}search.html\">Search the book</a></li>\n</ul>\n",
        ip = index_prefix,
    ));
    for (ci, chapter) in manifest.chapters.iter().enumerate() {
        let chapter_pages: Vec<&Page> = pages.iter().filter(|p| p.chapter_idx == ci).collect();
        if chapter_pages.is_empty() {
            continue;
        }
        let open = current.is_some_and(|c| chapter_pages.iter().any(|p| p.node.anchor == c));
        let mut items = String::new();
        for page in &chapter_pages {
            let cur = if current == Some(page.node.anchor.as_str()) {
                " aria-current=\"page\""
            } else {
                ""
            };
            items.push_str(&format!(
                "  <li><a href=\"{pp}{a}.html\"{cur} title=\"{full}\">{short}</a></li>\n",
                pp = page_prefix,
                a = html_escape(&page.node.anchor),
                full = nav_label(page_title(&page.node)),
                short = nav_label(&crate::short_label(&page.node)),
            ));
        }
        nav.push_str(&format!(
            "<details{o}><summary>{n}. {t} <span class=\"count\">{c}</span></summary>\n<ul>\n{items}</ul>\n</details>\n",
            o = if open { " open" } else { "" },
            n = ci + 1,
            t = html_escape(&chapter.title),
            c = chapter_pages.len(),
        ));
    }
    nav.push_str("</nav>");
    nav
}

fn page_title(node: &Node) -> &str {
    node.title.as_deref().unwrap_or(&node.anchor)
}

/// Resolve a manifest to the **ordered set of ingested documents** it names — the same resolution
/// (curated `sources` + drift-proof `globs` + `exclude`, and the one synthesized grammar EBNF)
/// [`build_book`] uses, exposed for **scoped emission** (`myc-doc build --manifest`, per-cluster PDF
/// export). Ingest the whole corpus first (so cross-references resolve against the *full* anchor
/// universe — the caller does this), then select and order exactly the manifest's documents.
///
/// # Errors
/// A manifest entry that resolves to no ingested document (or a duplicate/collision) is a
/// `BookError` — never a silently-empty subset (§4.1 "never a half-build"; G2).
pub fn resolve_manifest_docs(
    model: &DocModel,
    manifest: &BookManifest,
    repo_root: &Path,
) -> Result<Vec<Node>, BookError> {
    Ok(resolve_pages(model, manifest, repo_root)?
        .into_iter()
        .map(|p| p.node)
        .collect())
}

/// The manifest's chapters resolved to **(chapter-title, [ingested-doc anchors])**, best-effort over
/// the *corpus* model — the **semantic spine** for the corpus-site sidebar tree. Unlike
/// [`resolve_manifest_docs`], this is **lenient and never errors**: a chapter entry that is not an
/// ingested corpus document (a book-only page like `CONTRIBUTING.md` or the synthesized grammar EBNF,
/// which the corpus `build` does not include) is simply skipped — those pages exist in the *book*, not
/// the corpus site. Nothing corpus-side is dropped: every corpus doc still appears in the sidebar's
/// by-type groups regardless of the semantic spine (G2). `sources` keep their curated order; `globs`
/// are sorted by source path. Chapters that resolve to no corpus doc are omitted.
#[must_use]
pub fn resolve_manifest_chapters(
    model: &DocModel,
    manifest: &BookManifest,
) -> Vec<(String, Vec<String>)> {
    let by_source: BTreeMap<&str, &Node> = model
        .documents
        .iter()
        .map(|d| (d.provenance.source.as_str(), d))
        .collect();
    let mut out = Vec::new();
    for chapter in &manifest.chapters {
        let mut anchors = Vec::new();
        let mut seen = BTreeSet::new();
        for src in &chapter.sources {
            if let Some(n) = by_source.get(src.as_str()) {
                if seen.insert(n.anchor.clone()) {
                    anchors.push(n.anchor.clone());
                }
            }
        }
        if !chapter.globs.is_empty() {
            let mut matched: Vec<&Node> = model
                .documents
                .iter()
                .filter(|d| {
                    chapter
                        .globs
                        .iter()
                        .any(|g| glob_match(g, &d.provenance.source))
                })
                .filter(|d| !chapter.exclude.contains(&d.provenance.source))
                .collect();
            matched.sort_by(|a, b| a.provenance.source.cmp(&b.provenance.source));
            for n in matched {
                if seen.insert(n.anchor.clone()) {
                    anchors.push(n.anchor.clone());
                }
            }
        }
        if !anchors.is_empty() {
            out.push((chapter.title.clone(), anchors));
        }
    }
    out
}

/// Build every book artifact: the ToC/landing page, one page per chapter entry (prev/next nav), and
/// the search index + its page.
///
/// # Errors
/// A manifest entry that does not resolve to an ingested document (or a duplicate/collision) is a
/// `BookError` — a broken book is a build failure, never a silently-incomplete one (§4.1 "never a
/// half-build").
pub fn build_book(
    model: &DocModel,
    manifest: &BookManifest,
    repo_root: &Path,
) -> Result<Artifacts, BookError> {
    let pages = resolve_pages(model, manifest, repo_root)?;

    // Render every page's article through the SAME html renderer as the corpus site (composition,
    // not re-authorship) — scoped to exactly the book's pages.
    let scoped = DocModel::new(pages.iter().map(|p| p.node.clone()).collect());
    let rendered = crate::emit::html::render(&scoped, None);

    let mut arts = Artifacts::new();

    // ── per-page HTML, with prev/next + chapter breadcrumb ──────────────────────────────────────
    for (i, page) in pages.iter().enumerate() {
        let full_page = rendered
            .files
            .get(&format!("pages/{}.html", page.node.anchor))
            .map_or("", String::as_str);
        let article = extract_article(full_page);
        let chapter = &manifest.chapters[page.chapter_idx];

        let prev_link = i
            .checked_sub(1)
            .map(|j| &pages[j])
            .map(|p| {
                format!(
                    "<a href=\"{}.html\">← {}</a>",
                    html_escape(&p.node.anchor),
                    nav_label(page_title(&p.node))
                )
            })
            .unwrap_or_else(|| "<a href=\"../index.html\">← Table of contents</a>".to_owned());
        let next_link = pages
            .get(i + 1)
            .map(|p| {
                format!(
                    "<a href=\"{}.html\">{} →</a>",
                    html_escape(&p.node.anchor),
                    nav_label(page_title(&p.node))
                )
            })
            .unwrap_or_else(|| "<a href=\"../index.html\">Table of contents →</a>".to_owned());

        let breadcrumb = format!(
            "<p class=\"crumb\"><a href=\"../index.html\">Table of contents</a> · Chapter {}: {}</p>",
            page.chapter_idx + 1,
            html_escape(&chapter.title)
        );
        let main = format!(
            "{breadcrumb}\n{article}\n\
             <div class=\"pager\"><span>{prev_link}</span><span>{next_link}</span></div>"
        );
        let sidebar = book_sidebar(manifest, &pages, "", "../", Some(&page.node.anchor));
        arts.put(
            format!("book/pages/{}.html", page.node.anchor),
            book_shell(&manifest.title, page_title(&page.node), &sidebar, &main),
        );
    }

    // The chapter-tree sidebar for the root-level pages (ToC + search): pages live under `pages/`,
    // the ToC/search are siblings at `book/` root.
    let index_sidebar = book_sidebar(manifest, &pages, "pages/", "", None);

    // ── the ToC / landing page ──────────────────────────────────────────────────────────────────
    let mut toc = String::from("<nav aria-label=\"Table of contents\">\n");
    for (ci, chapter) in manifest.chapters.iter().enumerate() {
        toc.push_str(&format!(
            "<section><h2>{}. {}</h2>\n<ol>\n",
            ci + 1,
            html_escape(&chapter.title)
        ));
        for page in pages.iter().filter(|p| p.chapter_idx == ci) {
            toc.push_str(&format!(
                "  <li><a href=\"pages/{a}.html\" data-cid=\"{cid}\">{t}</a></li>\n",
                a = html_escape(&page.node.anchor),
                cid = html_escape(page.node.id.as_str()),
                t = nav_label(page_title(&page.node)),
            ));
        }
        toc.push_str("</ol></section>\n");
    }
    toc.push_str("</nav>");
    let index_main = format!(
        "<h1>{title}</h1>\n<p>{preface}</p>\n{toc}\n\
         <p>{chapters} chapters, {pages} pages. <a href=\"search.html\">Search the book</a>.</p>",
        title = html_escape(&manifest.title),
        preface = html_escape(&manifest.preface),
        chapters = manifest.chapters.len(),
        pages = pages.len(),
    );
    arts.put(
        "book/index.html",
        book_document(
            &manifest.title,
            &manifest.title,
            "",
            &index_sidebar,
            &index_main,
        ),
    );

    // ── search index + search page (client-side, no new dep) ───────────────────────────────────
    let search_index = render_search_index(&pages, manifest);
    arts.put("book/search-index.json", search_index);
    arts.put("book/assets/search.js", SEARCH_JS.to_owned());
    let search_main = format!("<h1>Search the book</h1>\n{SEARCH_PAGE_BODY}");
    arts.put(
        "book/search.html",
        book_document(&manifest.title, "Search", "", &index_sidebar, &search_main),
    );

    Ok(arts)
}

/// One search record — title, the chapter it lives in, its page URL (relative to `book/`), and a
/// short snippet (the document's lead prose, when present — grounded, never invented).
#[derive(Debug, Serialize)]
struct SearchRecord<'a> {
    /// The page's display title with inline markdown stripped (plain text; the JSON serializer owns
    /// the string escaping) — so literal `**`/backticks never leak into the book search UI.
    title: String,
    chapter: &'a str,
    url: String,
    snippet: String,
}

fn lead_snippet(node: &Node) -> String {
    let mut snippet = String::new();
    node.walk(&mut |n| {
        if snippet.is_empty() {
            if let Payload::Prose { text } = &n.payload {
                snippet = text.chars().take(200).collect();
            }
        }
    });
    snippet
}

fn render_search_index(pages: &[Page], manifest: &BookManifest) -> String {
    let records: Vec<SearchRecord<'_>> = pages
        .iter()
        .map(|p| SearchRecord {
            title: inline::to_plain(page_title(&p.node)),
            chapter: &manifest.chapters[p.chapter_idx].title,
            url: format!("pages/{}.html", p.node.anchor),
            snippet: lead_snippet(&p.node),
        })
        .collect();
    serde_json::to_string_pretty(&records).expect("search records are always serializable")
}

/// A small, dependency-free client-side substring filter over `search-index.json` — no search
/// engine, no heavy dep; keeps the crate's dependency posture (KC-3).
const SEARCH_JS: &str = "\
async function mycBookSearch() {
  const res = await fetch('search-index.json');
  const records = await res.json();
  const input = document.getElementById('book-search-box');
  const out = document.getElementById('book-search-results');
  function render(q) {
    out.innerHTML = '';
    if (!q) { return; }
    const needle = q.toLowerCase();
    const hits = records.filter(r =>
      r.title.toLowerCase().includes(needle) ||
      r.chapter.toLowerCase().includes(needle) ||
      r.snippet.toLowerCase().includes(needle)
    );
    for (const r of hits) {
      const li = document.createElement('li');
      const a = document.createElement('a');
      a.href = r.url;
      a.textContent = r.title + ' (' + r.chapter + ')';
      li.appendChild(a);
      out.appendChild(li);
    }
    if (hits.length === 0) {
      out.innerHTML = '<li>No matches.</li>';
    }
  }
  input.addEventListener('input', () => render(input.value));
}
mycBookSearch();
";

const SEARCH_PAGE_BODY: &str = "\
<p>Search across every page in the book (title, chapter, and lead summary — client-side, no \
server round-trip).</p>
<input id=\"book-search-box\" type=\"search\" placeholder=\"Search the book…\" \
aria-label=\"Search the book\">
<ul id=\"book-search-results\" aria-live=\"polite\"></ul>
<script src=\"assets/search.js\"></script>";
