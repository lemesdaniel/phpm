//! Materializes a project's vendor/ from the global store via hard links.

pub mod materialize;
pub mod reconcile;
pub mod sentinel;
pub mod volume;

use crate::materialize::{materialize_package, LinkMode};
use crate::reconcile::current_vendor_packages;
use crate::sentinel::{read_sentinel, write_sentinel};
use crate::volume::same_volume;
use lockfile::ComposerLock;
use std::collections::BTreeSet;
use std::path::Path;
use store::{PackageCoords, Store};

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Outcome of a sync. `no_op` is true when the sentinel matched and nothing was touched.
/// Carries counts only; M5 reconstructs per-package install/remove lines from a lock diff, not from this report.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub materialized: usize,
    pub removed: usize,
    pub no_op: bool,
}

/// Make `<project_dir>/vendor/` match `lock` exactly, materializing packages from `store`.
pub fn sync(
    project_dir: &Path,
    lock: &ComposerLock,
    store: &Store,
) -> Result<SyncReport, LinkError> {
    let vendor = project_dir.join("vendor");
    std::fs::create_dir_all(&vendor)?;

    // Encodes both content_hash and whether dev packages are present so a full→no-dev (or
    // no-dev→full) switch on the same lock is a cache miss that triggers a full reconcile.
    let sentinel_key = format!(
        "{}|dev={}",
        lock.content_hash,
        !lock.packages_dev.is_empty()
    );

    // Fast path: the sentinel records this exact lock → assume materialized, skip the walk.
    // An empty content_hash (lock missing it) is never trusted, else it would mask all changes.
    if !lock.content_hash.is_empty()
        && read_sentinel(&vendor)?.as_deref() == Some(sentinel_key.as_str())
    {
        return Ok(SyncReport {
            no_op: true,
            ..Default::default()
        });
    }

    // Decide hard-link vs copy once. Different volumes can't share inodes → copy, losing dedup.
    let mode = if same_volume(store.root_for_volume_check(), &vendor).unwrap_or(false) {
        LinkMode::HardLink
    } else {
        eprintln!(
            "phpm: warning: store and project are on different volumes; copying instead of hard-linking (no disk dedup). Set PHPM_STORE_DIR on the project's volume."
        );
        LinkMode::Copy
    };

    let mut desired: BTreeSet<(String, String)> = BTreeSet::new();
    let mut report = SyncReport::default();
    for locked in lock.packages.iter().chain(lock.packages_dev.iter()) {
        let coords = match PackageCoords::from_name(&locked.name, &locked.version) {
            Some(c) => c,
            None => continue, // platform package (php, ext-*)
        };
        desired.insert((coords.vendor.clone(), coords.package.clone()));
        materialize_from_store(store, &vendor, &coords, mode, &mut report)?;
    }

    for (v, p) in current_vendor_packages(&vendor)? {
        if !desired.contains(&(v.clone(), p.clone())) {
            std::fs::remove_dir_all(vendor.join(&v).join(&p))?;
            report.removed += 1;
        }
    }

    // Sentinel written last: only exists once vendor matches the lock.
    write_sentinel(&vendor, &sentinel_key)?;
    Ok(report)
}

fn materialize_from_store(
    store: &Store,
    vendor: &Path,
    coords: &PackageCoords,
    mode: LinkMode,
    report: &mut SyncReport,
) -> Result<(), LinkError> {
    // shared lock: many projects may link the same package
    let _lock = store.lock_shared(coords)?;
    // shared lock excludes a concurrent exclusive acquire, so has() is stable here
    if !store.has(coords) {
        return Err(LinkError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "package {}/{}@{} not present in store",
                coords.vendor, coords.package, coords.version
            ),
        )));
    }
    let store_pkg = store.package_path(coords);
    let dest = vendor.join(&coords.vendor).join(&coords.package);
    let n = materialize_package(&store_pkg, &dest, mode)?;
    if n > 0 {
        report.materialized += 1;
    }
    Ok(())
}
