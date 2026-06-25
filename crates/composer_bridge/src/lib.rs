//! Bridges to the Composer CLI: dependency resolution (--no-install, so Composer never
//! touches vendor/) and script execution. All Composer interaction goes through here.

mod resolve;
mod scripts;

pub use resolve::{remove, require, update};
pub use scripts::run_script;

use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("failed to run {program}: {source}")]
    Spawn { program: String, source: std::io::Error },
    #[error("{program} {args:?} failed: {stderr}")]
    Failed { program: String, args: Vec<String>, stderr: String },
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
}

/// Abstracts process execution so the bridge is testable without Composer installed.
pub trait Runner {
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> Result<(), BridgeError>;
}

/// Runs real subprocesses. Production Runner.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> Result<(), BridgeError> {
        let out = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|source| BridgeError::Spawn { program: program.to_string(), source })?;
        if !out.status.success() {
            return Err(BridgeError::Failed {
                program: program.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            });
        }
        Ok(())
    }
}
