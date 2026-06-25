use crate::GcError;
use std::collections::BTreeSet;
use std::path::Path;
use store::{PackageCoords, Store};

/// Result of a gc analysis. `to_remove` is what `execute_gc` would delete.
pub struct GcPlan {
    pub to_remove: Vec<PackageCoords>,
    pub referenced_count: usize,
}

/// Compute the `(vendor, package, version)` set referenced by the given projects'
/// composer.lock files, then mark every stored package NOT in that set for removal.
/// Read-only analysis — nothing is deleted (gc defaults to dry-run). A project with no
/// composer.lock contributes no references (it is skipped, not an error).
pub fn plan_gc(store: &Store, project_dirs: &[String]) -> Result<GcPlan, GcError> {
    let mut referenced: BTreeSet<(String, String, String)> = BTreeSet::new();
    for dir in project_dirs {
        let lock_path = Path::new(dir).join("composer.lock");
        let raw = match std::fs::read_to_string(&lock_path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(GcError::Io(e)),
        };
        let lock = lockfile::parse_lock(&raw)?;
        for locked in lock.packages.iter().chain(lock.packages_dev.iter()) {
            if let Some(c) = PackageCoords::from_name(&locked.name, &locked.version) {
                referenced.insert((c.vendor, c.package, c.version));
            }
        }
    }

    let to_remove = store
        .list_packages()?
        .into_iter()
        .filter(|c| {
            !referenced.contains(&(c.vendor.clone(), c.package.clone(), c.version.clone()))
        })
        .collect();

    Ok(GcPlan { to_remove, referenced_count: referenced.len() })
}

/// Delete the planned packages. Each is removed only if its exclusive lock can be taken
/// (skips packages a concurrent install is linking). Returns the number actually removed.
pub fn execute_gc(store: &Store, plan: &GcPlan) -> Result<usize, GcError> {
    let mut removed = 0;
    for coords in &plan.to_remove {
        match store.try_lock_exclusive(coords)? {
            Some(_lock) => {
                store.remove_package(coords)?;
                removed += 1;
            }
            None => {
                eprintln!(
                    "phpm: gc skipped {}/{}@{} (in use)",
                    coords.vendor, coords.package, coords.version
                );
            }
        }
    }
    Ok(removed)
}
