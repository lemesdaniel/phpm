use crate::{shasum, zipx, AcquireError, Fetcher};
use lockfile::Dist;
use store::{PackageCoords, Store};

/// Baixa o dist (zip), verifica o shasum, extrai (strip do dir-raiz) para um
/// diretório temporário e escreve no store de forma atômica.
/// NOTA(M3): a url do dist não tem allowlist de scheme/host — possível SSRF se PHPM rodar em nuvem. Allowlist fica p/ M3.
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
            "tipo de dist não suportado: '{}' (apenas zip no M2)",
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
