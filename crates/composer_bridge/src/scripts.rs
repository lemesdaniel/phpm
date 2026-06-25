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
    // --no-plugins: phpm does not support Composer plugins (v1 non-goal). Without it, the
    // PluginManager bootstrap during run-script can reject a project's plugin and abort.
    runner.run(
        "composer",
        &["run-script", "--no-interaction", "--no-plugins", event],
        project_dir,
    )
}
