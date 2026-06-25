use crate::{BridgeError, Runner};
use std::path::Path;

/// `composer update --no-install --no-scripts` — re-resolve and rewrite composer.lock;
/// never touch vendor/.
// --no-scripts: phpm runs only post-autoload-dump itself (after generate). post-install-cmd /
// post-update-cmd (e.g. Laravel key:generate, storage:link) are NOT run — known limitation, see backlog.
pub fn update(runner: &dyn Runner, project_dir: &Path) -> Result<(), BridgeError> {
    runner.run(
        "composer",
        &["update", "--no-install", "--no-scripts", "--no-interaction"],
        project_dir,
    )
}

/// `composer require <pkgs> --no-install --no-scripts`.
pub fn require(
    runner: &dyn Runner,
    project_dir: &Path,
    packages: &[String],
) -> Result<(), BridgeError> {
    let mut args = vec![
        "require",
        "--no-install",
        "--no-scripts",
        "--no-interaction",
    ];
    args.extend(packages.iter().map(|s| s.as_str()));
    runner.run("composer", &args, project_dir)
}

/// `composer remove <pkgs> --no-install --no-scripts`.
pub fn remove(
    runner: &dyn Runner,
    project_dir: &Path,
    packages: &[String],
) -> Result<(), BridgeError> {
    let mut args = vec!["remove", "--no-install", "--no-scripts", "--no-interaction"];
    args.extend(packages.iter().map(|s| s.as_str()));
    runner.run("composer", &args, project_dir)
}
