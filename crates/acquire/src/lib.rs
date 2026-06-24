//! Package acquisition: downloads dist or clones git source into the global store.

pub mod dist;
mod fetch;
pub mod git;
pub mod shasum;
pub mod zipx;

pub use fetch::{Fetcher, HttpFetcher};

use lockfile::LockedPackage;
use store::{PackageCoords, Store};

#[derive(Debug, thiserror::Error)]
pub enum AcquireError {
    #[error("HTTP: {0}")]
    Http(String),
    #[error("shasum mismatch: expected {expected}, got {actual}")]
    Shasum { expected: String, actual: String },
    #[error("invalid zip: {0}")]
    Zip(String),
    #[error("git failed: {0}")]
    Git(String),
    #[error("package {0} has no usable dist or source")]
    NoSource(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
}

/// Ensures `pkg` is materialized in the store and intact.
/// - Platform packages (no slash in name, e.g. "php") are skipped.
/// - Acquires under an EXCLUSIVE lock (decision Q8): lock → check has()+verify() →
///   if intact, skip; otherwise download the dist (if url present) or clone the git source.
pub fn acquire_package(
    store: &Store,
    fetcher: &dyn Fetcher,
    pkg: &LockedPackage,
) -> Result<(), AcquireError> {
    let coords = match PackageCoords::from_name(&pkg.name, &pkg.version) {
        Some(c) => c,
        None => return Ok(()), // platform package (php, ext-*) → nothing to acquire
    };

    // Exclusive lock BEFORE checking/writing — prevents TOCTOU between parallel installs.
    let _lock = store.lock_exclusive(&coords)?;

    if store.has(&coords) {
        if store.verify(&coords).is_ok() {
            return Ok(());
        }
        // present but corrupted (sha mismatch / inconsistent metadata) →
        // remove to allow clean re-materialization (under the exclusive lock).
        store.remove_package(&coords)?;
    }

    if let Some(dist) = &pkg.dist {
        if dist.url.is_some() {
            return dist::acquire_dist(store, fetcher, &coords, dist);
        }
    }
    if let Some(source) = &pkg.source {
        if source.url.is_some() {
            return git::acquire_git(store, &coords, source);
        }
    }
    Err(AcquireError::NoSource(format!(
        "{}/{}@{}",
        coords.vendor, coords.package, coords.version
    )))
}
