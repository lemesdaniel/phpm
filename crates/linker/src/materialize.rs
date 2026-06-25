use crate::LinkError;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub use crate::volume::LinkMode;

/// Materialize the package tree at `store_pkg` into `dest` using `mode`.
/// Returns the number of files newly linked/copied (already-correct hard links are skipped).
pub fn materialize_package(
    store_pkg: &Path,
    dest: &Path,
    mode: LinkMode,
) -> Result<usize, LinkError> {
    let mut count = 0;
    for entry in WalkDir::new(store_pkg).follow_links(false) {
        let entry = entry.map_err(|e| LinkError::Io(std::io::Error::other(e)))?;
        let rel = entry
            .path()
            .strip_prefix(store_pkg)
            .map_err(|e| LinkError::Io(std::io::Error::other(e)))?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        match mode {
            LinkMode::HardLink => {
                if links_match(entry.path(), &target)? {
                    continue;
                }
                // remove any existing entry (regular file or dangling symlink) before linking
                if fs::symlink_metadata(&target).is_ok() {
                    fs::remove_file(&target)?;
                }
                if let Err(e) = fs::hard_link(entry.path(), &target) {
                    // cross-volume / hard-link-unsupported (Windows different volume,
                    // bind mounts) → degrade to a copy for this file; propagate the
                    // original error only if the copy also fails.
                    fs::copy(entry.path(), &target).map_err(|_| LinkError::Io(e))?;
                }
            }
            LinkMode::Copy => {
                // no inode-skip in Copy mode; idempotency is gated by the sentinel at the sync level
                fs::copy(entry.path(), &target)?;
            }
        }
        count += 1;
    }
    prune_extraneous(store_pkg, dest)?;
    Ok(count)
}

/// Remove entries in `dest` that are absent from the store tree. Additive linking alone
/// would leave files from a previous version behind on upgrade; this makes dest mirror
/// the store package exactly.
fn prune_extraneous(store_pkg: &Path, dest: &Path) -> Result<(), LinkError> {
    if !dest.exists() {
        return Ok(());
    }
    let mut keep: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
    for entry in WalkDir::new(store_pkg).follow_links(false) {
        let entry = entry.map_err(|e| LinkError::Io(std::io::Error::other(e)))?;
        let rel = entry
            .path()
            .strip_prefix(store_pkg)
            .map_err(|e| LinkError::Io(std::io::Error::other(e)))?;
        if !rel.as_os_str().is_empty() {
            keep.insert(rel.to_path_buf());
        }
    }
    let mut to_remove = Vec::new();
    for entry in WalkDir::new(dest).follow_links(false).min_depth(1) {
        let entry = entry.map_err(|e| LinkError::Io(std::io::Error::other(e)))?;
        let rel = entry
            .path()
            .strip_prefix(dest)
            .map_err(|e| LinkError::Io(std::io::Error::other(e)))?;
        if !keep.contains(rel) {
            to_remove.push((entry.path().to_path_buf(), entry.file_type().is_dir()));
        }
    }
    // deepest-first so a pruned subtree's children are handled before the parent
    for (path, is_dir) in to_remove.into_iter().rev() {
        if !path.exists() {
            continue; // already removed as part of a parent's remove_dir_all
        }
        if is_dir {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// True when `target` already exists and is the same inode as `source` (hard-link identity).
#[cfg(unix)]
fn links_match(source: &Path, target: &Path) -> Result<bool, LinkError> {
    use std::os::unix::fs::MetadataExt;
    let t = match fs::symlink_metadata(target) {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };
    if t.file_type().is_symlink() {
        return Ok(false);
    }
    let s = fs::metadata(source)?;
    Ok(s.dev() == t.dev() && s.ino() == t.ino())
}

#[cfg(not(unix))]
fn links_match(_source: &Path, _target: &Path) -> Result<bool, LinkError> {
    Ok(false)
}
