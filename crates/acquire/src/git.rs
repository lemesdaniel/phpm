use crate::AcquireError;
use lockfile::Source;
use std::process::Command;
use store::{PackageCoords, Store};

/// Clona o `source` git num diretório temporário, faz checkout da `reference`,
/// remove o `.git` e escreve o resultado no store.
pub fn acquire_git(
    store: &Store,
    coords: &PackageCoords,
    source: &Source,
) -> Result<(), AcquireError> {
    let url = source.url.as_deref().ok_or_else(|| {
        AcquireError::NoSource(format!("{}/{}", coords.vendor, coords.package))
    })?;

    // Defesa em profundidade junto com os `--`: rejeita url/reference começando com `-`,
    // que o parser de opções do git veria como flag mesmo sendo argumento posicional.
    if url.starts_with('-') {
        return Err(AcquireError::Git(format!("url git rejeitada (começa com '-'): {url}")));
    }
    if source.reference.starts_with('-') {
        return Err(AcquireError::Git(format!(
            "reference git rejeitada (começa com '-'): {}",
            source.reference
        )));
    }

    let staging = tempfile::TempDir::new()?;
    let checkout = staging.path().join("co");
    let checkout_str = checkout.to_string_lossy().into_owned();

    // protocol.ext.allow=never bloqueia ext::sh (RCE). Allowlist completa de protocolos fica p/ M3.
    // `--` separa flags de argumentos posicionais: impede que a `url` vinda do composer.lock
    // seja tratada como flag do git (argument injection, ex. --upload-pack=... → RCE).
    // TODO(M3): clone completo é lento p/ repos grandes; avaliar --filter/--depth + fetch do sha.
    run_git(
        &["-c", "protocol.ext.allow=never", "clone", "--quiet", "--", url, &checkout_str],
        None,
    )?;
    if !source.reference.is_empty() {
        // `--` DEPOIS da reference: garante que ela seja tratada como commit-ish (não pathspec)
        // e impede que uma reference começando com `-` seja interpretada como flag.
        run_git(
            &["-c", "advice.detachedHead=false", "checkout", "--quiet", &source.reference, "--"],
            Some(&checkout),
        )?;
    }

    // não levar o histórico git para o store
    let dot_git = checkout.join(".git");
    if dot_git.exists() {
        std::fs::remove_dir_all(&dot_git)?;
    }

    store.write_package(coords, &checkout)?;
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&std::path::Path>) -> Result<(), AcquireError> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd
        .output()
        .map_err(|e| AcquireError::Git(format!("falha ao executar git: {e}")))?;
    if !out.status.success() {
        return Err(AcquireError::Git(format!(
            "git {:?} falhou: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}
