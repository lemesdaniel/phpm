use crate::{shasum, zipx, AcquireError, Fetcher};
use lockfile::Dist;
use store::{PackageCoords, Store};

/// Downloads the dist (zip), verifies the shasum, extracts (stripping the root directory)
/// into a temporary directory, and writes to the store atomically.
/// NOTE(M3): the dist url has no scheme/host allowlist — potential SSRF if PHPM runs in the cloud. Allowlist deferred to M3.
pub fn acquire_dist(
    store: &Store,
    fetcher: &dyn Fetcher,
    coords: &PackageCoords,
    dist: &Dist,
) -> Result<(), AcquireError> {
    let url = dist.url.as_deref().ok_or_else(|| {
        AcquireError::NoSource(format!(
            "{}/{}@{}",
            coords.vendor, coords.package, coords.version
        ))
    })?;

    if !dist.dist_type.eq_ignore_ascii_case("zip") {
        return Err(AcquireError::Zip(format!(
            "unsupported dist type: '{}' (only zip in M2)",
            dist.dist_type
        )));
    }

    let bytes = fetcher.fetch(url)?;
    shasum::verify_sha1(&bytes, &dist.shasum)?;

    let staging = tempfile::TempDir::new()?;
    zipx::extract_strip_root(&bytes, staging.path())?;

    store.write_package(coords, staging.path())?;
    Ok(())
}
