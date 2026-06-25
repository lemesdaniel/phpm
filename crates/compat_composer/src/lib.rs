//! Generates Composer-compatible autoload, installed-state, and bin files into vendor/.
//! Target is functional compatibility (classes load, InstalledVersions works), not
//! byte-identical output: Composer embeds a per-project random hash in class names.

pub mod aggregate;
pub mod php_emit;

#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lockfile: {0}")]
    Lock(#[from] lockfile::LockError),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
}

// consumed by generate() in a later task
#[allow(dead_code)]
/// Composer's ClassLoader, bundled verbatim (MIT — see assets/ASSETS_LICENSE).
pub(crate) const CLASS_LOADER_PHP: &str = include_str!("../assets/ClassLoader.php");

// consumed by generate() in a later task
#[allow(dead_code)]
/// Composer's InstalledVersions runtime, bundled verbatim (MIT).
pub(crate) const INSTALLED_VERSIONS_PHP: &str = include_str!("../assets/InstalledVersions.php");
