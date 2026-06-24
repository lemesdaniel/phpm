use sha2::{Digest, Sha256};
use std::io;
use std::path::Path;
use walkdir::WalkDir;

/// Deterministic hash of a directory's contents: independent of FS creation
/// order and platform (paths sorted, `/`-normalized).
///
/// Each file contributes a length-prefixed relative path then length-prefixed
/// bytes; the length prefixes prevent collisions between different boundary
/// splits (e.g. "ab"+"c" vs "a"+"bc").
///
/// Assumes UTF-8 file names (Composer packages are ASCII in practice); does not
/// apply NFC/NFD Unicode normalization — out of scope for M1.
pub fn sha256_tree(root: &Path) -> io::Result<String> {
    let mut files: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(io::Error::other)?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_str()
            .ok_or_else(|| io::Error::other("non-UTF-8 path in package"))?
            .replace('\\', "/");
        files.push((rel, entry.path().to_path_buf()));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel, abs) in files {
        let bytes = std::fs::read(&abs)?;
        hasher.update((rel.len() as u64).to_le_bytes());
        hasher.update(rel.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}
