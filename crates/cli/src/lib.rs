//! phpm CLI library: install/gc orchestration over the M1-M4 crates. The binary (main.rs)
//! is a thin clap front-end over these functions.

pub mod install;

use std::path::{Path, PathBuf};
use store::Store;

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

pub struct GcReport {
    pub would_remove: usize,
    pub removed: usize,
}

/// Run gc against the registered projects. `prune = false` is a dry run (reports only).
/// Prunes missing project paths from the registry first (decision Q4). An empty registry
/// is a friendly no-op (reports 0), NOT an error — plan_gc's EmptyRegistry guard exists to
/// stop an empty referenced set from nuking the store, but the CLI surfaces it as "nothing
/// to do".
pub fn gc_run(
    store: &Store,
    registry_base: &Path,
    prune: bool,
) -> Result<GcReport, install::CliError> {
    let reg = gc::registry::Registry::new(registry_base);
    reg.prune_missing()?;
    let projects = reg.list()?;
    let plan = match gc::collect::plan_gc(store, &projects) {
        Ok(p) => p,
        Err(gc::GcError::EmptyRegistry) => {
            return Ok(GcReport {
                would_remove: 0,
                removed: 0,
            })
        }
        Err(e) => return Err(install::CliError::Gc(e)),
    };
    let would_remove = plan.to_remove.len();
    let removed = if prune {
        gc::collect::execute_gc(store, &plan)?
    } else {
        0
    };
    Ok(GcReport {
        would_remove,
        removed,
    })
}
