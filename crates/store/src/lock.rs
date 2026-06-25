use crate::{PackageCoords, Store, StoreError};
use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

/// RAII guard for a package lock. When dropped, releases the lock (flock is also
/// released automatically by the OS if the process dies — no orphan lock).
pub struct PackageLock {
    _file: File,
}

impl Store {
    fn lock_file_path(&self, coords: &PackageCoords) -> PathBuf {
        self.root_ref().join("locks").join(format!(
            "{}__{}__{}.lock",
            coords.vendor, coords.package, coords.version
        ))
    }

    fn open_lock_file(&self, coords: &PackageCoords) -> Result<File, StoreError> {
        let path = self.lock_file_path(coords);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?)
    }

    /// Shared lock — multiple simultaneous readers/linkers. Blocking.
    pub fn lock_shared(&self, coords: &PackageCoords) -> Result<PackageLock, StoreError> {
        let file = self.open_lock_file(coords)?;
        FileExt::lock_shared(&file)?;
        Ok(PackageLock { _file: file })
    }

    /// Exclusive lock — for writing to the store and for GC. Blocking.
    // M5 will add try_lock_exclusive so GC can skip packages currently in use.
    pub fn lock_exclusive(&self, coords: &PackageCoords) -> Result<PackageLock, StoreError> {
        let file = self.open_lock_file(coords)?;
        FileExt::lock_exclusive(&file)?;
        Ok(PackageLock { _file: file })
    }

    /// Try the exclusive lock without blocking. Ok(None) if contended (a shared/exclusive
    /// lock is held), Err on a real io failure. Used by gc to skip in-use packages.
    pub fn try_lock_exclusive(
        &self,
        coords: &PackageCoords,
    ) -> Result<Option<PackageLock>, StoreError> {
        let file = self.open_lock_file(coords)?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(true) => Ok(Some(PackageLock { _file: file })),
            Ok(false) => Ok(None),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    /// Tries to acquire the shared lock without blocking.
    /// Ok(None) = exclusive lock active (contention); Err = real I/O failure.
    pub fn try_lock_shared(
        &self,
        coords: &PackageCoords,
    ) -> Result<Option<PackageLock>, StoreError> {
        let file = self.open_lock_file(coords)?;
        match FileExt::try_lock_shared(&file) {
            Ok(true) => Ok(Some(PackageLock { _file: file })),
            Ok(false) => Ok(None),
            Err(e) => Err(StoreError::Io(e)),
        }
    }
}
