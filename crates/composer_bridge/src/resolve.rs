use crate::{BridgeError, Runner};
use std::path::Path;

/// `composer update --no-install` — re-resolve and rewrite composer.lock; never touch vendor/.
pub fn update(runner: &dyn Runner, project_dir: &Path) -> Result<(), BridgeError> {
    runner.run("composer", &["update", "--no-install", "--no-interaction"], project_dir)
}

/// `composer require <pkgs> --no-install`.
pub fn require(runner: &dyn Runner, project_dir: &Path, packages: &[String]) -> Result<(), BridgeError> {
    let mut args = vec!["require", "--no-install", "--no-interaction"];
    args.extend(packages.iter().map(|s| s.as_str()));
    runner.run("composer", &args, project_dir)
}

/// `composer remove <pkgs> --no-install`.
pub fn remove(runner: &dyn Runner, project_dir: &Path, packages: &[String]) -> Result<(), BridgeError> {
    let mut args = vec!["remove", "--no-install", "--no-interaction"];
    args.extend(packages.iter().map(|s| s.as_str()));
    runner.run("composer", &args, project_dir)
}
