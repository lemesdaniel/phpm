//! Generates Composer-compatible autoload, installed-state, and bin files into vendor/.
//! Target is functional compatibility (classes load, InstalledVersions works), not
//! byte-identical output: Composer embeds a per-project random hash in class names.

pub mod aggregate;
pub mod bin_proxies;
pub mod classmap;
pub mod installed;
pub mod php_emit;

use crate::aggregate::{aggregate_autoload, AutoloadData, PathBase};
use crate::installed::InstalledPackage;
use lockfile::{ComposerLock};
use std::collections::BTreeMap;
use std::path::Path;
use store::{PackageCoords, Store};

#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lockfile: {0}")]
    Lock(#[from] lockfile::LockError),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
}

/// Composer's ClassLoader, bundled verbatim (MIT — see assets/ASSETS_LICENSE).
pub(crate) const CLASS_LOADER_PHP: &str = include_str!("../assets/ClassLoader.php");

/// Composer's InstalledVersions runtime, bundled verbatim (MIT).
pub(crate) const INSTALLED_VERSIONS_PHP: &str = include_str!("../assets/InstalledVersions.php");

/// Fixed 32-hex autoload hash. Composer randomizes this per project; functional behavior
/// depends only on it being consistent across the generated files, so a constant is fine.
const AUTOLOAD_HASH: &str = "phpm00000000000000000000000000000";

/// Generate the Composer-compatible autoload, installed-state, and bin files into vendor/.
/// Run AFTER the linker has materialized vendor/<vendor>/<package>/.
pub fn generate(
    project_dir: &Path,
    lock: &ComposerLock,
    store: &Store,
    root_json: &str,
) -> Result<(), GenError> {
    let vendor = project_dir.join("vendor");
    let composer_dir = vendor.join("composer");
    let bin_dir = vendor.join("bin");
    std::fs::create_dir_all(&composer_dir)?;

    let root = lockfile::parse_json(root_json)?;

    let mut data = AutoloadData::default();
    aggregate_autoload(&mut data, &root, PathBase::Base, None);

    let mut classmap: BTreeMap<String, String> = BTreeMap::new();
    let mut installed: Vec<InstalledPackage> = Vec::new();
    let mut extras: BTreeMap<String, serde_json::Value> = BTreeMap::new();

    for locked in lock.packages.iter().chain(lock.packages_dev.iter()) {
        let coords = match PackageCoords::from_name(&locked.name, &locked.version) {
            Some(c) => c,
            None => continue,
        };
        let pkg_dir = vendor.join(&coords.vendor).join(&coords.package);
        let prefix = format!("{}/{}", coords.vendor, coords.package);

        if let Ok(raw) = std::fs::read_to_string(pkg_dir.join("composer.json")) {
            if let Ok(pj) = lockfile::parse_json(&raw) {
                aggregate_autoload(&mut data, &pj, PathBase::Vendor, Some(&prefix));
                classmap.extend(crate::classmap::classmap_for_package(store, &coords, &pkg_dir)?);
                // bin proxies declared by the package
                for bin in &pj.bin {
                    write_bin_proxy(&bin_dir, &prefix, bin)?;
                }
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(extra) = v.get("extra") {
                    extras.insert(locked.name.clone(), extra.clone());
                }
            }
        }

        installed.push(InstalledPackage {
            name: locked.name.clone(),
            version: locked.version.clone(),
            package_type: locked.package_type.clone(),
            reference: locked
                .dist
                .as_ref()
                .map(|d| d.reference.clone())
                .or_else(|| locked.source.as_ref().map(|s| s.reference.clone()))
                .unwrap_or_default(),
        });
    }

    std::fs::write(composer_dir.join("ClassLoader.php"), CLASS_LOADER_PHP)?;
    std::fs::write(composer_dir.join("InstalledVersions.php"), INSTALLED_VERSIONS_PHP)?;

    use crate::php_emit::*;
    std::fs::write(composer_dir.join("autoload_psr4.php"), render_psr4_php(&data.psr4))?;
    std::fs::write(composer_dir.join("autoload_namespaces.php"), render_psr0_php(&data.psr0))?;
    std::fs::write(composer_dir.join("autoload_files.php"), render_files_php(&data.files))?;
    std::fs::write(composer_dir.join("autoload_classmap.php"), render_classmap_php(&classmap))?;
    std::fs::write(composer_dir.join("autoload_real.php"), render_autoload_real(AUTOLOAD_HASH))?;
    std::fs::write(vendor.join("autoload.php"), render_autoload_entry(AUTOLOAD_HASH))?;

    let root_name = if root.name.is_empty() { "__root__".to_string() } else { root.name.clone() };
    std::fs::write(
        composer_dir.join("installed.php"),
        crate::installed::render_installed_php(&root_name, "1.0.0+no-version-set", &installed),
    )?;
    std::fs::write(
        composer_dir.join("installed.json"),
        crate::installed::render_installed_json(&installed, &extras),
    )?;

    Ok(())
}

/// Write a `vendor/bin/<tool>` PHP proxy (and a `.bat` on all platforms for portability),
/// pointing at the real binary at `vendor/<prefix>/<bin>`. The proxy gets the +x bit on unix.
fn write_bin_proxy(bin_dir: &Path, pkg_prefix: &str, bin_rel: &str) -> Result<(), GenError> {
    std::fs::create_dir_all(bin_dir)?;
    let tool = Path::new(bin_rel).file_name().and_then(|n| n.to_str()).unwrap_or(bin_rel);
    let rel_from_vendor = format!("{}/{}", pkg_prefix, bin_rel.trim_start_matches('/'));
    let proxy_path = bin_dir.join(tool);
    std::fs::write(&proxy_path, crate::bin_proxies::render_bin_proxy_php(&rel_from_vendor))?;
    std::fs::write(bin_dir.join(format!("{tool}.bat")), crate::bin_proxies::render_bin_proxy_bat(tool))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&proxy_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&proxy_path, perms)?;
    }
    Ok(())
}
