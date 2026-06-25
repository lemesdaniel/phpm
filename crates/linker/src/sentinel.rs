use crate::LinkError;
use serde::{Deserialize, Serialize};
use std::path::Path;

const SENTINEL: &str = ".phpm-state";

#[derive(Serialize, Deserialize)]
struct State {
    content_hash: String,
}

/// Read the lock content-hash recorded in `vendor/.phpm-state`, if present and parseable.
/// A missing file (or missing vendor dir) returns Ok(None). A present-but-corrupt file
/// also returns Ok(None) so sync falls back to a full reconcile rather than erroring.
pub fn read_sentinel(vendor: &Path) -> Result<Option<String>, LinkError> {
    let path = vendor.join(SENTINEL);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(LinkError::Io(e)),
    };
    Ok(serde_json::from_str::<State>(&raw)
        .ok()
        .map(|s| s.content_hash))
}

/// Write the lock content-hash as the completion marker. Must be the LAST step of sync,
/// so the sentinel only exists once all links are materialized.
pub fn write_sentinel(vendor: &Path, content_hash: &str) -> Result<(), LinkError> {
    let state = State {
        content_hash: content_hash.to_string(),
    };
    std::fs::write(vendor.join(SENTINEL), serde_json::to_vec(&state)?)?;
    Ok(())
}
