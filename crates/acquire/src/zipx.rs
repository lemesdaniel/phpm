use crate::AcquireError;
use std::io::{Cursor, Read};
use std::path::Path;

/// Extracts a zip (bytes) into `dest`, stripping the single root directory that
/// Composer/Packagist archives wrap their contents in (e.g. `vendor-pkg-<hash>/`).
/// If the files do NOT share a single root directory, extracts without stripping.
pub fn extract_strip_root(zip_bytes: &[u8], dest: &Path) -> Result<(), AcquireError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| AcquireError::Zip(e.to_string()))?;

    let root = common_root(&mut archive)?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AcquireError::Zip(e.to_string()))?;
        let name = file.name().to_string();
        // native check for path traversal / absolute paths (component level)
        if file.enclosed_name().is_none() {
            return Err(AcquireError::Zip(format!("unsafe entry: {name}")));
        }
        // symlinks in archive: rejected (M2). zip stores the target as file content;
        // materializing it as a regular file would corrupt the package.
        if file.is_symlink() {
            return Err(AcquireError::Zip(format!("symlink rejected: {name}")));
        }
        let rel = match &root {
            Some(prefix) => name.strip_prefix(prefix.as_str()).unwrap_or(&name),
            None => name.as_str(),
        };
        if rel.is_empty() {
            continue; // the root directory entry itself
        }
        let out = dest.join(rel);
        if file.is_dir() || name.ends_with('/') {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let cap = (file.size() as usize).min(64 * 1024 * 1024);
            let mut bytes = Vec::with_capacity(cap);
            file.read_to_end(&mut bytes)
                .map_err(|e| AcquireError::Zip(e.to_string()))?;
            std::fs::write(&out, bytes)?;
        }
    }
    Ok(())
}

/// Returns `Some("<root>/")` if ALL entries start with the same first
/// component; otherwise `None` (no stripping performed).
fn common_root<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Option<String>, AcquireError> {
    let mut root: Option<String> = None;
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| AcquireError::Zip(e.to_string()))?;
        let name = file.name();
        let first = match name.split_once('/') {
            Some((head, _)) if !head.is_empty() => head.to_string(),
            _ => return Ok(None),
        };
        match &root {
            None => root = Some(first),
            Some(r) if *r != first => return Ok(None),
            _ => {}
        }
    }
    Ok(root.map(|r| format!("{r}/")))
}
