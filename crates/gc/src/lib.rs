//! Garbage collection of unreferenced store packages, plus the project registry that
//! tracks which projects reference what.

pub mod collect;
pub mod registry;

#[derive(Debug, thiserror::Error)]
pub enum GcError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("store: {0}")]
    Store(#[from] store::StoreError),
    #[error("lockfile: {0}")]
    Lock(#[from] lockfile::LockError),
    #[error("no projects registered; refusing to plan gc (it would treat the entire store as garbage)")]
    EmptyRegistry,
}
