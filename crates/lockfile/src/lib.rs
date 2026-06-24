//! Parsing de composer.lock e composer.json. Sem I/O.

mod json;
mod lock;

pub use json::{Autoload, ComposerJson};
pub use lock::{ComposerLock, Dist, LockedPackage, Source};

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn parse_lock(input: &str) -> Result<ComposerLock, LockError> {
    Ok(serde_json::from_str(input)?)
}

pub fn parse_json(input: &str) -> Result<ComposerJson, LockError> {
    Ok(serde_json::from_str(input)?)
}
