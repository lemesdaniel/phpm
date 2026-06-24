//! Parsing de composer.lock e composer.json. Sem I/O.

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComposerLock {
    pub content_hash: String,
    pub packages: Vec<()>,
}

pub fn parse_lock(_input: &str) -> Result<ComposerLock, LockError> {
    unimplemented!()
}
