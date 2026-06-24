//! Aquisição de pacotes: baixa dist ou clona git source para o store global.

mod fetch;
pub mod shasum;
pub mod zipx;

pub use fetch::{Fetcher, HttpFetcher};

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
