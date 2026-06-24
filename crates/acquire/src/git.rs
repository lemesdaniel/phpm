use crate::AcquireError;
use lockfile::Source;
use std::process::Command;
use store::{PackageCoords, Store};

/// Clones the git `source` into a temporary directory, checks out `reference`,
/// removes `.git`, and writes the result to the store.
pub fn acquire_git(
    store: &Store,
    coords: &PackageCoords,
    source: &Source,
) -> Result<(), AcquireError> {
    let url = source.url.as_deref().ok_or_else(|| {
        AcquireError::NoSource(format!(
            "{}/{}@{}",
            coords.vendor, coords.package, coords.version
        ))
    })?;

    // Defense-in-depth alongside the `--` separators: reject url/reference starting with `-`,
    // which git's option parser would interpret as a flag even when passed as a positional argument.
    if url.starts_with('-') {
        return Err(AcquireError::Git(format!(
            "git url rejected (starts with '-'): {url}"
        )));
    }
    if source.reference.starts_with('-') {
        return Err(AcquireError::Git(format!(
            "git reference rejected (starts with '-'): {}",
            source.reference
        )));
    }

    if !source.source_type.eq_ignore_ascii_case("git") {
        return Err(AcquireError::Git(format!(
            "unsupported source type: '{}' (only git in M2)",
            source.source_type
        )));
    }

    let staging = tempfile::TempDir::new()?;
    let checkout = staging.path().join("co");
    let checkout_str = checkout.to_string_lossy().into_owned();

    // protocol.ext.allow=never blocks ext::sh (RCE). Full protocol allowlist deferred to M3.
    // `--` separates flags from positional arguments: prevents the `url` from composer.lock
    // from being treated as a git flag (argument injection, e.g. --upload-pack=... → RCE).
    // TODO(M3): full clone is slow for large repos; evaluate --filter/--depth + sha fetch.
    run_git(
        &[
            "-c",
            "protocol.ext.allow=never",
            "clone",
            "--quiet",
            "--",
            url,
            &checkout_str,
        ],
        None,
    )?;
    if !source.reference.is_empty() {
        // `--` AFTER the reference: ensures it is treated as a commit-ish (not a pathspec)
        // and prevents a reference starting with `-` from being interpreted as a flag.
        run_git(
            &[
                "-c",
                "advice.detachedHead=false",
                "checkout",
                "--quiet",
                &source.reference,
                "--",
            ],
            Some(&checkout),
        )?;
    }

    // do not carry git history into the store
    let dot_git = checkout.join(".git");
    if dot_git.exists() {
        std::fs::remove_dir_all(&dot_git)?;
    }

    store.write_package(coords, &checkout)?;
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&std::path::Path>) -> Result<(), AcquireError> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    // private repos that require auth fail fast instead of hanging on an interactive prompt.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    let out = cmd
        .output()
        .map_err(|e| AcquireError::Git(format!("failed to execute git: {e}")))?;
    if !out.status.success() {
        return Err(AcquireError::Git(format!(
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}
