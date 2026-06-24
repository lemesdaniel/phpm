//! Materializes a project's vendor/ from the global store via hard links.

pub mod materialize;
pub mod sentinel;
pub mod volume;

use lockfile::ComposerLock;
use std::path::Path;
use store::Store;

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
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub materialized: usize,
    pub removed: usize,
    pub no_op: bool,
}

/// Make `<project_dir>/vendor/` match `lock` exactly, materializing packages from `store`.
pub fn sync(project_dir: &Path, lock: &ComposerLock, store: &Store) -> Result<SyncReport, LinkError> {
    let vendor = project_dir.join("vendor");
    std::fs::create_dir_all(&vendor)?;
    let _ = (lock, store);
    Ok(SyncReport::default())
}
