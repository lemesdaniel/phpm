use std::io;
use std::path::Path;

/// How files are materialized into vendor/.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkMode {
    /// Hard link from the store (no data duplication). Same volume only.
    HardLink,
    /// Plain copy (different volume / hard links unsupported). Loses dedup.
    Copy,
}

/// True when both paths live on the same filesystem (hard links cannot cross volumes).
#[cfg(unix)]
pub fn same_volume(a: &Path, b: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(std::fs::metadata(a)?.dev() == std::fs::metadata(b)?.dev())
}

/// Windows: device identity needs a volume serial via file handle; until M3 hardening
/// we report "same volume" and rely on a copy fallback when a hard link actually fails.
#[cfg(windows)]
pub fn same_volume(_a: &Path, _b: &Path) -> io::Result<bool> {
    Ok(true)
}

#[cfg(not(any(unix, windows)))]
pub fn same_volume(_a: &Path, _b: &Path) -> io::Result<bool> {
    Ok(false)
}
