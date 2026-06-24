use crate::LinkError;
use std::path::Path;

/// `composer` and `bin` under vendor/ are managed by autoload/bin generation (M4), not packages.
const RESERVED: &[&str] = &["composer", "bin"];

/// List the (vendor, package) pairs currently materialized under `vendor/`, scanning the
/// two-level `vendor/<vendor>/<package>` layout. Reserved dirs and dotfiles are skipped.
/// A missing vendor dir yields an empty list.
pub fn current_vendor_packages(vendor: &Path) -> Result<Vec<(String, String)>, LinkError> {
    let mut out = Vec::new();
    let vendors = match std::fs::read_dir(vendor) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(LinkError::Io(e)),
    };
    for vendor_entry in vendors {
        let vendor_entry = vendor_entry?;
        if !vendor_entry.file_type()?.is_dir() {
            continue;
        }
        let vname = vendor_entry.file_name().to_string_lossy().into_owned();
        if vname.starts_with('.') || RESERVED.contains(&vname.as_str()) {
            continue;
        }
        for pkg_entry in std::fs::read_dir(vendor_entry.path())? {
            let pkg_entry = pkg_entry?;
            if !pkg_entry.file_type()?.is_dir() {
                continue;
            }
            let pname = pkg_entry.file_name().to_string_lossy().into_owned();
            out.push((vname.clone(), pname));
        }
    }
    Ok(out)
}
