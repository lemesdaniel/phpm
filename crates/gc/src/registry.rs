use crate::GcError;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Tracks which project directories phpm knows about, so gc can compute which store
/// packages are still referenced. One absolute path per line in `<base>/projects`
/// (base is typically ~/.phpm).
pub struct Registry {
    file: PathBuf,
}

impl Registry {
    pub fn new(base: &Path) -> Self {
        Registry {
            file: base.join("projects"),
        }
    }

    /// Add a project path (idempotent — deduped).
    pub fn register(&self, project_dir: &str) -> Result<(), GcError> {
        let mut set = self.read_set()?;
        set.insert(project_dir.to_string());
        self.write_set(&set)
    }

    pub fn list(&self) -> Result<Vec<String>, GcError> {
        Ok(self.read_set()?.into_iter().collect())
    }

    /// Drop registered paths that no longer exist on disk.
    pub fn prune_missing(&self) -> Result<(), GcError> {
        let set: BTreeSet<String> = self
            .read_set()?
            .into_iter()
            .filter(|p| Path::new(p).exists())
            .collect();
        self.write_set(&set)
    }

    fn read_set(&self) -> Result<BTreeSet<String>, GcError> {
        match std::fs::read_to_string(&self.file) {
            Ok(s) => Ok(s
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
            Err(e) => Err(GcError::Io(e)),
        }
    }

    fn write_set(&self, set: &BTreeSet<String>) -> Result<(), GcError> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = set.iter().cloned().collect::<Vec<_>>().join("\n");
        std::fs::write(&self.file, body)?;
        Ok(())
    }
}
