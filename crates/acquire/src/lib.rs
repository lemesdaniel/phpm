//! Aquisição de pacotes: baixa dist ou clona git source para o store global.

mod fetch;
pub mod dist;
pub mod git;
pub mod shasum;
pub mod zipx;

pub use fetch::{Fetcher, HttpFetcher};

use lockfile::LockedPackage;
use store::{PackageCoords, Store};

#[derive(Debug, thiserror::Error)]
pub enum AcquireError {
    #[error("HTTP: {0}")]
    Http(String),
    #[error("shasum não confere: esperado {expected}, obtido {actual}")]
    Shasum { expected: String, actual: String },
    #[error("zip inválido: {0}")]
    Zip(String),
    #[error("git falhou: {0}")]
    Git(String),
    #[error("pacote {0} não tem dist nem source utilizáveis")]
    NoSource(String),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
}

/// Garante que `pkg` esteja materializado no store, íntegro.
/// - Pacotes de plataforma (sem barra no nome, ex. "php") são ignorados.
/// - Adquire sob lock EXCLUSIVO (decisão Q8): lock → checa has()+verify() →
///   se íntegro, pula; senão baixa o dist (se houver url) ou clona o git source.
pub fn acquire_package(
    store: &Store,
    fetcher: &dyn Fetcher,
    pkg: &LockedPackage,
) -> Result<(), AcquireError> {
    let coords = match PackageCoords::from_name(&pkg.name, &pkg.version) {
        Some(c) => c,
        None => return Ok(()), // plataforma (php, ext-*) → nada a adquirir
    };

    // lock exclusivo ANTES de checar/escrever — evita TOCTOU entre installs paralelos.
    let _lock = store.lock_exclusive(&coords)?;

    if store.has(&coords) {
        if store.verify(&coords).is_ok() {
            return Ok(());
        }
        // presente mas corrompido (sha diverge / meta inconsistente) →
        // remove para permitir re-materialização limpa (sob o lock exclusivo).
        store.remove_package(&coords)?;
    }

    if let Some(dist) = &pkg.dist {
        if dist.url.is_some() {
            return dist::acquire_dist(store, fetcher, &coords, dist);
        }
    }
    if let Some(source) = &pkg.source {
        if source.url.is_some() {
            return git::acquire_git(store, &coords, source);
        }
    }
    Err(AcquireError::NoSource(format!("{}/{}", coords.vendor, coords.package)))
}
