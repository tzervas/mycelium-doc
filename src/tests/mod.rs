//! In-crate white-box test modules (test layout rule, CLAUDE.md: no tests in logic files; one
//! submodule per source module under test). This crate's inline `#[cfg(test)] mod tests` blocks
//! elsewhere are pre-existing (M-797 lazy-retrofit debt, not yet swept); these modules are
//! extracted **as-touched** — `build.rs` (extended with `extra_md_files`, §`crate::book`), the
//! brand-new `book.rs`, `ir.rs` (extracted when `Node::walk` was guarded, RFC-0041 W1/§4.7),
//! `apiref.rs` (extracted when the `=>` return-arrow split bug was fixed, M-1004), the
//! brand-new `lib_index.rs` (M-1004), and `html.rs`/`typst.rs` (extracted when the readability
//! theme + syntax highlighting + print code-legibility pass landed) — plus the brand-new
//! `highlight.rs` and `cache.rs`, which start clean here rather than compounding the debt.

mod apiref;
mod book;
mod build;
mod cache;
mod highlight;
mod html;
mod inline;
mod ir;
mod label;
mod lib_index;
mod typst;
