use crate::{PackageCoords, Store, StoreError};
use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;

/// Guarda RAII de um lock de pacote. Ao dropar, libera o lock (flock também é
/// liberado automaticamente pelo SO se o processo morrer — sem lock órfão).
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

    /// Lock compartilhado — múltiplos leitores/linkers simultâneos. Bloqueia.
    pub fn lock_shared(&self, coords: &PackageCoords) -> Result<PackageLock, StoreError> {
        let file = self.open_lock_file(coords)?;
        FileExt::lock_shared(&file)?;
        Ok(PackageLock { _file: file })
    }

    /// Lock exclusivo — para escrita no store e para o GC. Bloqueia.
    // M5 adicionará try_lock_exclusive para o GC pular pacotes em uso.
    pub fn lock_exclusive(&self, coords: &PackageCoords) -> Result<PackageLock, StoreError> {
        let file = self.open_lock_file(coords)?;
        FileExt::lock_exclusive(&file)?;
        Ok(PackageLock { _file: file })
    }

    /// Tenta o lock compartilhado sem bloquear.
    /// Ok(None) = exclusive ativo (contenção); Err = falha real de io.
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
