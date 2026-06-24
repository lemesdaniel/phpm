use crate::AcquireError;
use sha1::{Digest, Sha1};

/// Verifies the sha1 of the dist bytes against the `shasum` from composer.lock.
/// If `expected` is empty (common for GitHub zipballs), skips verification —
/// integrity of the extracted content is still enforced by the store's sha256.
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
