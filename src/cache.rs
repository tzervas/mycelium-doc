//! **Differential rendering cache** — incremental re-emit keyed on the emitted bytes' content hash.
//!
//! A build is a *pure function* of the content-addressed model ([`crate::ir::DocModel`]) and the one
//! template, so a page's rendered bytes change **iff** its projected content (its node subtree's
//! content address — ADR-003) or the shared template changes. This cache records `path → blake3(bytes)`
//! for every emitted artifact; on the next build it **skips writing** any file whose hash is unchanged
//! and whose on-disk copy is still present. Provenance is *not* hashed (ADR-003), so re-flowing a
//! source line does not perturb the hash — stable content ⇒ stable bytes ⇒ stable hash ⇒ skip.
//!
//! Hashing the *emitted bytes* (rather than the node id alone) is deliberate and more correct: a
//! change to the shared CSS template leaves every node id unchanged but changes every page's bytes —
//! a node-id-only key would wrongly skip them. The byte hash captures *both* content and template
//! changes, so a stale page can never be silently kept (G2).
//!
//! ## Never-silent (G2)
//! A **missing, unreadable, unparseable, or version-incompatible** cache is not an error — it degrades
//! to a **full rebuild** with an explicit [`EmitReport::notice`] the CLI prints (never a silent stale
//! skip). `--full` / `--no-cache` forces a full rebuild (the prior cache is ignored) while still
//! writing a fresh cache so the *next* build is incremental. Artifacts recorded by a prior build but
//! absent now (a removed doc's orphaned page) are **removed and counted**, never left as dead files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::emit::Artifacts;

/// The on-disk cache filename (written inside the output directory).
pub const CACHE_FILE: &str = ".myc-doc-cache.json";

/// The cache format version. Bumped if the entry semantics change; a mismatch ⇒ full rebuild
/// (never-silent — reported, never a wrong skip).
pub const CACHE_VERSION: u32 = 1;

/// The persisted cache: `emitted-path → blake3(bytes)`, plus the format version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffCache {
    /// The format version (see [`CACHE_VERSION`]).
    pub version: u32,
    /// Emitted artifact path (out-relative) → its content hash (`blake3:<hex>` of the exact bytes).
    pub entries: BTreeMap<String, String>,
}

impl DiffCache {
    /// The cache computed from a freshly-rendered artifact set (before any write).
    #[must_use]
    pub fn of(arts: &Artifacts) -> DiffCache {
        DiffCache {
            version: CACHE_VERSION,
            entries: arts
                .files
                .iter()
                .map(|(path, bytes)| (path.clone(), hash_bytes(bytes)))
                .collect(),
        }
    }

    /// Load a prior cache from `out_dir/.myc-doc-cache.json`. Never-silent: a missing / unreadable /
    /// unparseable / version-mismatched cache yields `None` (⇒ full rebuild), never an error.
    #[must_use]
    pub fn load(out_dir: &Path) -> Option<DiffCache> {
        let src = std::fs::read_to_string(out_dir.join(CACHE_FILE)).ok()?;
        let cache: DiffCache = serde_json::from_str(&src).ok()?;
        (cache.version == CACHE_VERSION).then_some(cache)
    }

    /// Persist this cache to `out_dir/.myc-doc-cache.json`.
    ///
    /// # Errors
    /// Propagates the write error (with its path) — a cache we cannot persist is surfaced, not
    /// swallowed (the emit itself has already succeeded; this only affects the *next* build).
    pub fn store(&self, out_dir: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).expect("DiffCache is always serializable");
        let path = out_dir.join(CACHE_FILE);
        std::fs::write(&path, json)
            .map_err(|e| std::io::Error::new(e.kind(), format!("writing {}: {e}", path.display())))
    }
}

/// The `blake3:<hex>` content hash of `bytes` (the kernel's content-address shape).
#[must_use]
fn hash_bytes(bytes: &str) -> String {
    format!("blake3:{}", blake3::hash(bytes.as_bytes()).to_hex())
}

/// What an incremental emit did — every count explicit, for a never-silent CLI summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitReport {
    /// Artifacts (re)written because they were new or changed.
    pub written: usize,
    /// Artifacts skipped because their content hash was unchanged and the file was present.
    pub skipped: usize,
    /// Orphaned prior artifacts removed (a doc that no longer exists).
    pub removed: usize,
    /// Total artifacts in the current build.
    pub total: usize,
    /// A never-silent notice (e.g. why the cache was ignored), if any.
    pub notice: Option<String>,
}

/// Write `arts` under `out_dir` incrementally, using (and refreshing) the differential cache.
///
/// With `use_cache` true, only new/changed artifacts are written and unchanged ones are skipped;
/// with it false (`--full` / `--no-cache`) every artifact is written. Either way the fresh cache is
/// stored so the *next* build can be incremental. Orphaned prior artifacts are removed.
///
/// # Errors
/// Propagates the first filesystem error (with its path) — never a silent partial write.
pub fn emit_incremental(
    arts: &Artifacts,
    out_dir: &Path,
    use_cache: bool,
) -> std::io::Result<EmitReport> {
    std::fs::create_dir_all(out_dir).map_err(|e| {
        std::io::Error::new(e.kind(), format!("creating {}: {e}", out_dir.display()))
    })?;

    let fresh = DiffCache::of(arts);
    let mut notice: Option<String> = None;
    let prior = if use_cache {
        match DiffCache::load(out_dir) {
            Some(c) => Some(c),
            None => {
                // Distinguish "no cache yet" from "cache present but ignored" for an honest notice.
                if out_dir.join(CACHE_FILE).exists() {
                    notice = Some(
                        "prior cache was unreadable or version-incompatible — full rebuild"
                            .to_owned(),
                    );
                }
                None
            }
        }
    } else {
        notice = Some("cache bypassed (--full/--no-cache) — full rebuild".to_owned());
        None
    };

    let (mut written, mut skipped) = (0usize, 0usize);
    for (rel, contents) in &arts.files {
        let target = out_dir.join(rel);
        let unchanged = prior
            .as_ref()
            .and_then(|p| p.entries.get(rel))
            .is_some_and(|h| h == fresh.entries.get(rel).expect("fresh has every artifact"));
        if unchanged && target.exists() {
            skipped += 1;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                std::io::Error::new(e.kind(), format!("creating {}: {e}", parent.display()))
            })?;
        }
        std::fs::write(&target, contents).map_err(|e| {
            std::io::Error::new(e.kind(), format!("writing {}: {e}", target.display()))
        })?;
        written += 1;
    }

    // Remove orphans: artifacts the prior cache emitted that are absent now (a removed doc). Never
    // silent — removals are counted, and a removal that FAILS is reported in the notice (G2), never
    // dropped. The cache file itself is never an artifact, so never an orphan.
    let mut removed = 0usize;
    let mut failed: Vec<String> = Vec::new();
    if let Some(p) = &prior {
        let current: BTreeSet<&String> = arts.files.keys().collect();
        for old in p.entries.keys() {
            if !current.contains(old) {
                let path = out_dir.join(old);
                if !path.exists() {
                    continue; // already gone
                }
                match std::fs::remove_file(&path) {
                    Ok(()) => removed += 1,
                    Err(e) => failed.push(format!("{old} ({e})")),
                }
            }
        }
    }
    if !failed.is_empty() {
        let msg = format!(
            "could not remove {} orphaned artifact(s): {}",
            failed.len(),
            failed.join(", ")
        );
        notice = Some(match notice {
            Some(n) => format!("{n}; {msg}"),
            None => msg,
        });
    }

    fresh.store(out_dir)?;

    Ok(EmitReport {
        written,
        skipped,
        removed,
        total: arts.files.len(),
        notice,
    })
}
