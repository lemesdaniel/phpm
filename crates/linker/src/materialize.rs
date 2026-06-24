use crate::LinkError;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub use crate::volume::LinkMode;

/// Materialize the package tree at `store_pkg` into `dest` using `mode`.
/// Returns the number of files newly linked/copied (already-correct hard links are skipped).
pub fn materialize_package(store_pkg: &Path, dest: &Path, mode: LinkMode) -> Result<usize, LinkError> {
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
                if target.exists() {
                    fs::remove_file(&target)?;
                }
                fs::hard_link(entry.path(), &target)?;
            }
            LinkMode::Copy => {
                fs::copy(entry.path(), &target)?;
            }
        }
        count += 1;
    }
    Ok(count)
}

/// True when `target` already exists and is the same inode as `source` (hard-link identity).
#[cfg(unix)]
fn links_match(source: &Path, target: &Path) -> Result<bool, LinkError> {
    use std::os::unix::fs::MetadataExt;
    if !target.exists() {
        return Ok(false);
    }
    let s = fs::metadata(source)?;
    let t = fs::metadata(target)?;
    Ok(s.dev() == t.dev() && s.ino() == t.ino())
}

#[cfg(not(unix))]
fn links_match(_source: &Path, _target: &Path) -> Result<bool, LinkError> {
    Ok(false)
}
