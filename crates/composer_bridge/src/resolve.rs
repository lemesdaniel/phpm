use crate::{BridgeError, Runner};
use std::path::Path;

/// `composer update --no-install --no-scripts --no-plugins`: re-resolve and rewrite
/// composer.lock; never touch vendor/.
// --no-scripts: phpm runs only post-autoload-dump itself (after generate). post-install-cmd /
// post-update-cmd (e.g. Laravel key:generate, storage:link) are NOT run, known limitation, see backlog.
// --no-plugins: Composer plugins are a v1 non-goal; skipping their bootstrap avoids aborts.
pub fn update(runner: &dyn Runner, project_dir: &Path) -> Result<(), BridgeError> {
    runner.run(
        "composer",
        &[
            "update",
            "--no-install",
            "--no-scripts",
            "--no-plugins",
            "--no-interaction",
        ],
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
        "--no-plugins",
        "--no-interaction",
    ];
    args.extend(packages.iter().map(|s| s.as_str()));
    runner.run("composer", &args, project_dir)
}

/// `composer remove <pkgs> --no-install --no-scripts --no-plugins`.
pub fn remove(
    runner: &dyn Runner,
    project_dir: &Path,
    packages: &[String],
) -> Result<(), BridgeError> {
    let mut args = vec![
        "remove",
        "--no-install",
        "--no-scripts",
        "--no-plugins",
        "--no-interaction",
    ];
    args.extend(packages.iter().map(|s| s.as_str()));
    runner.run("composer", &args, project_dir)
}
