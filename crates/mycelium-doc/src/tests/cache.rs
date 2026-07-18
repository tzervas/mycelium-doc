//! White-box tests for [`crate::cache`] — the differential rendering cache. Covers the skip-unchanged
//! / rewrite-changed / remove-orphan behaviour and the never-silent full-rebuild fallbacks.

use std::path::PathBuf;

use crate::cache::{emit_incremental, DiffCache, CACHE_FILE};
use crate::emit::Artifacts;

fn temp_dir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("mycdoc-cache-{tag}-{nanos}"));
    p
}

fn arts_of(pairs: &[(&str, &str)]) -> Artifacts {
    let mut a = Artifacts::new();
    for (k, v) in pairs {
        a.put(*k, *v);
    }
    a
}

#[test]
fn first_build_writes_everything_and_stores_a_cache() {
    let dir = temp_dir("first");
    let arts = arts_of(&[("index.html", "<x>"), ("pages/a.html", "A")]);

    let report = emit_incremental(&arts, &dir, true).expect("emit");
    assert_eq!(report.written, 2);
    assert_eq!(report.skipped, 0);
    assert_eq!(report.total, 2);
    assert!(dir.join("index.html").exists());
    assert!(dir.join(CACHE_FILE).exists());

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_second_identical_build_skips_every_unchanged_page() {
    let dir = temp_dir("skip");
    let arts = arts_of(&[("index.html", "<x>"), ("pages/a.html", "A")]);
    emit_incremental(&arts, &dir, true).expect("first");

    let report = emit_incremental(&arts, &dir, true).expect("second");
    assert_eq!(report.written, 0, "nothing changed");
    assert_eq!(report.skipped, 2, "both skipped by content hash");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn only_the_changed_page_is_rewritten() {
    let dir = temp_dir("change");
    emit_incremental(
        &arts_of(&[("index.html", "<x>"), ("pages/a.html", "A")]),
        &dir,
        true,
    )
    .expect("first");

    // `pages/a.html` changes content; `index.html` is identical.
    let report = emit_incremental(
        &arts_of(&[("index.html", "<x>"), ("pages/a.html", "A2")]),
        &dir,
        true,
    )
    .expect("second");
    assert_eq!(report.written, 1, "only the changed page");
    assert_eq!(report.skipped, 1);
    assert_eq!(
        std::fs::read_to_string(dir.join("pages/a.html")).unwrap(),
        "A2"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_orphaned_page_is_removed_never_left_as_a_dead_file() {
    let dir = temp_dir("orphan");
    emit_incremental(
        &arts_of(&[("index.html", "<x>"), ("pages/gone.html", "G")]),
        &dir,
        true,
    )
    .expect("first");
    assert!(dir.join("pages/gone.html").exists());

    // The next build no longer emits `pages/gone.html`.
    let report = emit_incremental(&arts_of(&[("index.html", "<x>")]), &dir, true).expect("second");
    assert_eq!(report.removed, 1, "the orphan is removed");
    assert!(
        !dir.join("pages/gone.html").exists(),
        "dead file cleaned up"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_failed_orphan_removal_is_reported_never_silent() {
    let dir = temp_dir("orphanfail");
    emit_incremental(
        &arts_of(&[("index.html", "<x>"), ("pages/gone.html", "G")]),
        &dir,
        true,
    )
    .expect("first");
    // Make the orphan path un-removable by `remove_file`: replace the file with a directory.
    let orphan = dir.join("pages/gone.html");
    std::fs::remove_file(&orphan).unwrap();
    std::fs::create_dir(&orphan).unwrap();

    let report = emit_incremental(&arts_of(&[("index.html", "<x>")]), &dir, true).expect("emit");
    assert_eq!(
        report.removed, 0,
        "the un-removable orphan is not counted removed"
    );
    assert!(
        report.notice.as_deref().unwrap_or("").contains("gone.html"),
        "the removal failure is reported, never silent: {:?}",
        report.notice
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_cache_forces_a_full_rebuild_with_a_notice() {
    let dir = temp_dir("full");
    let arts = arts_of(&[("index.html", "<x>"), ("pages/a.html", "A")]);
    emit_incremental(&arts, &dir, true).expect("first");

    // `use_cache = false` (--full/--no-cache): write everything, and say so (never-silent).
    let report = emit_incremental(&arts, &dir, false).expect("full");
    assert_eq!(report.written, 2, "full rebuild rewrites all");
    assert_eq!(report.skipped, 0);
    assert!(
        report.notice.is_some(),
        "the bypass is announced, not silent"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_corrupt_or_missing_cache_degrades_to_a_full_rebuild() {
    let dir = temp_dir("corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    // Missing cache → None (full rebuild).
    assert!(DiffCache::load(&dir).is_none());
    // Corrupt cache → None (never a wrong skip).
    std::fs::write(dir.join(CACHE_FILE), "{not valid json").unwrap();
    assert!(DiffCache::load(&dir).is_none());

    // And an incremental build over a corrupt cache writes everything, with a notice.
    let report = emit_incremental(&arts_of(&[("index.html", "<x>")]), &dir, true).expect("emit");
    assert_eq!(report.written, 1);
    assert!(report.notice.is_some());

    std::fs::remove_dir_all(&dir).ok();
}
