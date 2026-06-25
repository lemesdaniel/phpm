use crate::{BridgeError, Runner};
use std::path::Path;

/// Run a Composer script event via `composer run-script`, but ONLY if the project's
/// composer.json declares that event. Running an undeclared event would make Composer
/// error ("script not defined"); skipping is the correct no-op. This is how Laravel's
/// package:discover (post-autoload-dump) is triggered after generate().
pub fn run_script(runner: &dyn Runner, project_dir: &Path, event: &str) -> Result<(), BridgeError> {
    let raw = match std::fs::read_to_string(project_dir.join("composer.json")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(BridgeError::Io(e)),
    };
    let declared = lockfile::parse_json(&raw)
        .map(|cj| cj.scripts.contains_key(event))
        .unwrap_or(false);
    if !declared {
        return Ok(());
    }
    // Plugins ARE allowed here: event plugins (e.g. a phpcs standards installer) hook script
    // events like post-autoload-dump and must run. Composer only activates plugins the project
    // lists in config.allow-plugins, and installed.json now carries each package's full require
    // block so the PluginManager can validate them. Installer plugins that relocate install
    // paths are still not honored because phpm, not Composer, materializes vendor/.
    runner.run(
        "composer",
        &["run-script", "--no-interaction", event],
        project_dir,
    )
}
