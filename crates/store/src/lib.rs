//! Global store for PHPM: layout, integrity, atomicity, and locks.

mod atomic;
mod hash;
mod lock;
mod verify;
pub use hash::sha256_tree;
pub use lock::PackageLock;

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("package already exists in store: {0}")]
    AlreadyExists(String),
    #[error("integrity failure: expected {expected}, got {actual}")]
    Integrity { expected: String, actual: String },
    #[error("missing metadata for {0}")]
    MissingMeta(String),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCoords {
    pub vendor: String,
    pub package: String,
    pub version: String,
}

impl PackageCoords {
    /// Converts a Composer "vendor/package" name + version into coords.
    /// Returns None for platform names (no slash, e.g. "php", "ext-json").
    pub fn from_name(name: &str, version: &str) -> Option<Self> {
        let (vendor, package) = name.split_once('/')?;
        if vendor.is_empty() || package.is_empty() || package.contains('/') {
            return None;
        }
        Some(PackageCoords {
            vendor: vendor.to_string(),
            package: package.to_string(),
            version: version.to_string(),
        })
    }

    fn namespace_rel(&self) -> PathBuf {
        Path::new(&self.vendor).join(&self.package)
    }

    fn rel(&self) -> PathBuf {
        self.namespace_rel().join(&self.version)
    }
}

#[derive(Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Store {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn package_path(&self, coords: &PackageCoords) -> PathBuf {
        self.root.join("packages").join(coords.rel())
    }

    pub fn meta_path(&self, coords: &PackageCoords) -> PathBuf {
        // NOTE: Do NOT use .with_extension("json") here — it would replace the
        // last component of the version string. For "3.8.1", the OS extension is
        // "1", so .with_extension("json") would yield "3.8.json" instead of
        // "3.8.1.json". We append ".json" explicitly instead.
        self.root
            .join("meta")
            .join(coords.namespace_rel())
            .join(format!("{}.json", coords.version))
    }

    pub fn has(&self, coords: &PackageCoords) -> bool {
        self.package_path(coords).is_dir()
    }

    pub(crate) fn root_ref(&self) -> &std::path::Path {
        &self.root
    }

    /// Root path for volume comparison (hard links cannot cross filesystems).
    pub fn root_for_volume_check(&self) -> &std::path::Path {
        self.root_ref()
    }

    /// Enumerate every `(vendor, package, version)` materialized under `packages/`.
    /// A missing store returns an empty list.
    pub fn list_packages(&self) -> Result<Vec<PackageCoords>, StoreError> {
        let root = self.root_ref().join("packages");
        let mut out = Vec::new();
        let vendors = match std::fs::read_dir(&root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(StoreError::Io(e)),
        };
        for v in vendors {
            let v = v?;
            if !v.file_type()?.is_dir() { continue; }
            let vendor = v.file_name().to_string_lossy().into_owned();
            for p in std::fs::read_dir(v.path())? {
                let p = p?;
                if !p.file_type()?.is_dir() { continue; }
                let package = p.file_name().to_string_lossy().into_owned();
                for ver in std::fs::read_dir(p.path())? {
                    let ver = ver?;
                    if !ver.file_type()?.is_dir() { continue; }
                    out.push(PackageCoords {
                        vendor: vendor.clone(),
                        package: package.clone(),
                        version: ver.file_name().to_string_lossy().into_owned(),
                    });
                }
            }
        }
        Ok(out)
    }
}
