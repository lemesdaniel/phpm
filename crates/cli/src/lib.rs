//! phpm CLI library: install/gc orchestration over the M1-M4 crates. The binary (main.rs)
//! is a thin clap front-end over these functions.

pub mod install;

use std::path::PathBuf;

/// Resolve the store directory: `$PHPM_STORE_DIR` if set, else `~/.phpm/store`.
/// (Decision Q9: configurable so CI can place it on the project's volume.)
pub fn store_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PHPM_STORE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".phpm").join("store")
}

/// Base dir for the project registry: `~/.phpm`.
pub fn registry_base() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".phpm")
}
