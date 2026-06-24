//! Store global do PHPM: layout, integridade, atomicidade e locks.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("pacote já existe no store: {0}")]
    AlreadyExists(String),
    #[error("falha de integridade: esperado {expected}, obtido {actual}")]
    Integrity { expected: String, actual: String },
    #[error("metadados ausentes para {0}")]
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
    /// Converte um nome Composer "vendor/package" + versão em coords.
    /// Retorna None para nomes de plataforma (sem barra, ex. "php", "ext-json").
    pub fn from_name(name: &str, version: &str) -> Option<Self> {
        let (vendor, package) = name.split_once('/')?;
        Some(PackageCoords {
            vendor: vendor.to_string(),
            package: package.to_string(),
            version: version.to_string(),
        })
    }

    fn rel(&self) -> PathBuf {
        Path::new(&self.vendor).join(&self.package).join(&self.version)
    }
}

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
            .join(&coords.vendor)
            .join(&coords.package)
            .join(format!("{}.json", coords.version))
    }

    pub fn has(&self, coords: &PackageCoords) -> bool {
        self.package_path(coords).is_dir()
    }
}
