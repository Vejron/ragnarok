//! A download cache, so a rebuild of the world does not fetch the same data again.
//!
//! Everything the importer downloads goes through here: the Overpass answer, the tree register,
//! the STAC searches, and every 512 KiB block read from Lantmäteriet's height model and point
//! cloud. Entries are files in one directory (`.cache/osm-import` by default, git-ignored),
//! named by what they hold and a hash of the request. Credentials are never part of a key or an
//! entry. Nothing expires: `--refresh` re-downloads and overwrites, `--clear-cache` deletes it all.
//!
//! The cache is process-wide, set once by `init`; until then (and in tests) it is off.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

struct Cache {
    dir: PathBuf,
    /// Re-download everything, replacing what is cached.
    refresh: bool,
}

static CACHE: OnceLock<Option<Cache>> = OnceLock::new();
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);
static BYTES_DOWNLOADED: AtomicU64 = AtomicU64::new(0);

/// Turns the cache on in `dir`, or leaves it off with `None`. Call once, before any download.
pub fn init(dir: Option<PathBuf>, refresh: bool) {
    let _ = CACHE.set(dir.map(|dir| Cache { dir, refresh }));
}

/// Deletes the cache directory. Returns how many bytes it held.
pub fn clear(dir: &Path) -> Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }
    let size = size_of(dir);
    std::fs::remove_dir_all(dir).with_context(|| format!("clearing {}", dir.display()))?;
    Ok(size)
}

fn size_of(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}

/// FNV-1a, 64 bit: names entries, so the key itself (a URL or a query) never has to be a filename.
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// The bytes for `(kind, key)`: from the cache if present, else from `download`, which is then
/// stored. `kind` is a short readable prefix for the file; `key` is everything that makes the
/// request unique, and must not contain credentials.
pub fn bytes(kind: &str, key: &str, download: impl FnOnce() -> Result<Vec<u8>>) -> Result<Vec<u8>> {
    let Some(Some(cache)) = CACHE.get() else {
        return download();
    };
    let path = cache.dir.join(format!("{kind}-{:016x}", hash(key)));
    if !cache.refresh {
        if let Ok(found) = std::fs::read(&path) {
            HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(found);
        }
    }
    MISSES.fetch_add(1, Ordering::Relaxed);
    let fresh = download()?;
    BYTES_DOWNLOADED.fetch_add(fresh.len() as u64, Ordering::Relaxed);
    // Written under another name and renamed, so an interrupted run never leaves a short entry.
    let store = || -> std::io::Result<()> {
        std::fs::create_dir_all(&cache.dir)?;
        let partial = path.with_extension("part");
        std::fs::write(&partial, &fresh)?;
        std::fs::rename(&partial, &path)
    };
    if let Err(error) = store() {
        eprintln!("  cache: could not store {}: {error}", path.display());
    }
    Ok(fresh)
}

/// `bytes` for UTF-8 answers (JSON).
pub fn text(kind: &str, key: &str, download: impl FnOnce() -> Result<String>) -> Result<String> {
    let raw = bytes(kind, key, || download().map(String::into_bytes))?;
    String::from_utf8(raw).context("cached entry is not UTF-8; try --refresh")
}

/// One line for the end of a run, or `None` when the cache is off.
pub fn summary() -> Option<String> {
    let Some(Some(cache)) = CACHE.get() else { return None };
    let (hits, misses) = (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed));
    let downloaded = BYTES_DOWNLOADED.load(Ordering::Relaxed);
    Some(format!(
        "cache {}: {hits} reused, {misses} downloaded ({} MiB)",
        cache.dir.display(),
        downloaded / (1024 * 1024)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable_and_distinguish_keys() {
        assert_eq!(hash("a"), hash("a"));
        assert_ne!(hash("a"), hash("b"));
        assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn with_the_cache_off_every_call_downloads() {
        // The test binary never calls `init`, so this is the uncached path.
        let mut calls = 0;
        for _ in 0..2 {
            bytes("t", "k", || {
                calls += 1;
                Ok(vec![1])
            })
            .unwrap();
        }
        assert_eq!(calls, 2);
    }

    #[test]
    fn clearing_removes_the_directory_and_reports_its_size() {
        let dir = std::env::temp_dir().join(format!("osm-import-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a"), [0u8; 10]).unwrap();
        assert_eq!(clear(&dir).unwrap(), 10);
        assert!(!dir.exists());
        assert_eq!(clear(&dir).unwrap(), 0, "clearing a missing cache is fine");
    }
}
