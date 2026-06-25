use acquire::Fetcher;
use composer_bridge::Runner;
use std::path::{Path, PathBuf};
use store::Store;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("lockfile: {0}")]
    Lock(#[from] lockfile::LockError),
    #[error("acquire: {0}")]
    Acquire(#[from] acquire::AcquireError),
    #[error("link: {0}")]
    Link(#[from] linker::LinkError),
    #[error("generate: {0}")]
    Gen(#[from] compat_composer::GenError),
    #[error("composer: {0}")]
    Bridge(#[from] composer_bridge::BridgeError),
    #[error("gc: {0}")]
    Gc(#[from] gc::GcError),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("no composer.lock found (run `phpm update` first to resolve dependencies)")]
    NoLock,
}

pub struct InstallOpts {
    /// Base dir for the project registry (typically ~/.phpm).
    pub registry_base: PathBuf,
    /// When true, packages_dev are excluded from acquire/link/generate.
    pub no_dev: bool,
}

/// Result of the install pipeline. Callers may inspect `lock_possibly_stale`
/// and emit a warning; it is a heuristic — a version-only drift is NOT detected.
pub struct InstallReport {
    /// True when composer.json requires at least one real (non-platform) package
    /// that is absent from composer.lock. Run `phpm update` to re-resolve.
    pub lock_possibly_stale: bool,
}

/// The install pipeline: acquire → link → generate → scripts → register.
/// `fetcher`/`runner` are injected so this is testable offline. Assumes composer.lock
/// exists (the CLI layer resolves first when it is missing/stale).
pub fn install(
    project_dir: &Path,
    store: &Store,
    fetcher: &dyn Fetcher,
    runner: &dyn Runner,
    opts: &InstallOpts,
) -> Result<InstallReport, CliError> {
    let lock_raw = match std::fs::read_to_string(project_dir.join("composer.lock")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(CliError::NoLock),
        Err(e) => return Err(CliError::Io(e)),
    };
    let mut lock = lockfile::parse_lock(&lock_raw)?;

    // Staleness check: computed from the FULL lock (both sections) before any --no-dev clear,
    // so a dev require is not falsely flagged stale under --no-dev.
    let locked_names: std::collections::BTreeSet<String> = lock
        .packages
        .iter()
        .chain(lock.packages_dev.iter())
        .map(|p| p.name.clone())
        .collect();
    let lock_possibly_stale = {
        let root_raw =
            std::fs::read_to_string(project_dir.join("composer.json")).unwrap_or_default();
        match lockfile::parse_json(&root_raw) {
            Ok(cj) => cj
                .require
                .keys()
                .chain(cj.require_dev.keys())
                .filter(|name| store::PackageCoords::from_name(name, "0").is_some())
                .any(|name| !locked_names.contains(name)),
            Err(_) => false,
        }
    };

    if opts.no_dev {
        lock.packages_dev.clear();
    }

    for locked in lock.packages.iter().chain(lock.packages_dev.iter()) {
        acquire::acquire_package(store, fetcher, locked)?;
    }

    linker::sync(project_dir, &lock, store)?;

    let root_json = match std::fs::read_to_string(project_dir.join("composer.json")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_string(),
        Err(e) => return Err(CliError::Io(e)),
    };
    compat_composer::generate(project_dir, &lock, store, &root_json)?;

    composer_bridge::run_script(runner, project_dir, "post-autoload-dump")?;

    let reg = gc::registry::Registry::new(&opts.registry_base);
    let project_str = project_dir.to_str().ok_or_else(|| {
        CliError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "project path is not valid UTF-8",
        ))
    })?;
    reg.register(project_str)?;

    Ok(InstallReport {
        lock_possibly_stale,
    })
}
