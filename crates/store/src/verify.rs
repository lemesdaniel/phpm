use crate::hash::sha256_tree;
use crate::{PackageCoords, Store, StoreError};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct MetaRead {
    sha256: String,
}

impl Store {
    /// Recomputes the sha256 of the package tree and compares it against the stored meta.
    /// Ok(()) if intact. Errors with MissingMeta if the meta does not exist, Integrity if they diverge.
    pub fn verify(&self, coords: &PackageCoords) -> Result<(), StoreError> {
        let meta_path = self.meta_path(coords);
        // "missing meta" (NotFound) aligns with the self-heal in write_package (dir without meta = partial install).
        let meta_raw = fs::read_to_string(&meta_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StoreError::MissingMeta(format!(
                    "{}/{}@{}",
                    coords.vendor, coords.package, coords.version
                ))
            } else {
                StoreError::Io(e)
            }
        })?;
        let meta: MetaRead = serde_json::from_str(&meta_raw)?;
        let actual = sha256_tree(&self.package_path(coords)).map_err(StoreError::Io)?;
        if actual != meta.sha256 {
            return Err(StoreError::Integrity {
                expected: meta.sha256,
                actual,
            });
        }
        Ok(())
    }
}
