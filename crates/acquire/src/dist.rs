use crate::{shasum, zipx, AcquireError, Fetcher};
use lockfile::Dist;
use store::{PackageCoords, Store};

/// Baixa o dist (zip), verifica o shasum, extrai (strip do dir-raiz) para um
/// diretório temporário e escreve no store de forma atômica.
pub fn acquire_dist(
    store: &Store,
    fetcher: &dyn Fetcher,
    coords: &PackageCoords,
    dist: &Dist,
) -> Result<(), AcquireError> {
    let url = dist
        .url
        .as_deref()
        .ok_or_else(|| AcquireError::NoSource(format!("{}/{}", coords.vendor, coords.package)))?;

    let bytes = fetcher.fetch(url)?;
    shasum::verify_sha1(&bytes, &dist.shasum)?;

    let staging = tempfile::TempDir::new()?;
    zipx::extract_strip_root(&bytes, staging.path())?;

    store.write_package(coords, staging.path())?;
    Ok(())
}
