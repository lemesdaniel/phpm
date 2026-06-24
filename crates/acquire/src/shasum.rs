use crate::AcquireError;
use sha1::{Digest, Sha1};

/// Verifica o sha1 dos bytes do dist contra o `shasum` do composer.lock.
/// Se `expected` for vazio (comum em zipballs do GitHub), pula a verificação —
/// a integridade do conteúdo extraído ainda é travada pelo sha256 do store.
pub fn verify_sha1(bytes: &[u8], expected: &str) -> Result<(), AcquireError> {
    if expected.is_empty() {
        return Ok(());
    }
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    let actual = hex::encode(hasher.finalize());
    if actual != expected.to_lowercase() {
        return Err(AcquireError::Shasum {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}
