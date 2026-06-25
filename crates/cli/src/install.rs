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
) -> Result<(), CliError> {
    let lock_raw = match std::fs::read_to_string(project_dir.join("composer.lock")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(CliError::NoLock),
        Err(e) => return Err(CliError::Io(e)),
    };
    let lock = lockfile::parse_lock(&lock_raw)?;

    for locked in lock.packages.iter().chain(lock.packages_dev.iter()) {
        acquire::acquire_package(store, fetcher, locked)?;
    }

    linker::sync(project_dir, &lock, store)?;

    let root_json = std::fs::read_to_string(project_dir.join("composer.json")).unwrap_or_else(|_| "{}".into());
    compat_composer::generate(project_dir, &lock, store, &root_json)?;

    composer_bridge::run_script(runner, project_dir, "post-autoload-dump")?;

    let reg = gc::registry::Registry::new(&opts.registry_base);
    reg.register(project_dir.to_str().unwrap_or_default())?;

    Ok(())
}
