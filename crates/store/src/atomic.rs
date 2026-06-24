use crate::hash::sha256_tree;
use crate::{PackageCoords, Store, StoreError};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
struct Meta {
    name: String,
    version: String,
    sha256: String,
}

impl Store {
    /// Escreve um pacote já extraído (em `src_dir`) no store de forma atômica:
    /// copia para um diretório temporário dentro do store, fsync, e move com
    /// rename atômico para o destino final. Em seguida escreve o meta json.
    /// Erra com AlreadyExists se o destino já existe (caller deve checar has()
    /// sob lock antes de chamar).
    pub fn write_package(&self, coords: &PackageCoords, src_dir: &Path) -> Result<(), StoreError> {
        let dest = self.package_path(coords);
        if dest.is_dir() {
            if self.meta_path(coords).exists() {
                // instalação completa → não reescreve
                return Err(StoreError::AlreadyExists(format!(
                    "{}/{}@{}",
                    coords.vendor, coords.package, coords.version
                )));
            }
            // dir presente mas sem meta = instalação parcial (crash entre rename e meta).
            // Remove o órfão e re-materializa. (Sob lock exclusivo quando Task 10 existir.)
            // O dir pode estar read-only (crash pós-imutabilidade): restaura escrita antes de remover.
            set_writable_recursive(&dest)?;
            fs::remove_dir_all(&dest)?;
        }

        let tmp_root = self.root_ref().join("tmp");
        fs::create_dir_all(&tmp_root)?;
        // diretório temporário único por (coords) — sufixo determinístico simples;
        // a unicidade real entre processos é garantida pelo lock exclusivo (Task 10).
        // TODO(Task 10): nome de staging seguro entre processos exige lock exclusivo
        let staging = tmp_root.join(format!(
            "{}__{}__{}.staging",
            coords.vendor, coords.package, coords.version
        ));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        copy_tree(src_dir, &staging)?;
        fsync_tree(&staging)?;

        // calcula integridade ANTES de aplicar read-only
        let sha = sha256_tree(&staging).map_err(StoreError::Io)?;

        // garante o diretório-pai do destino
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // rename atômico staging → destino
        fs::rename(&staging, &dest)?;

        // imutabilidade: store é read-only. Write em vendor/ (mesmo inode via
        // hard link em M3) falha alto em vez de corromper o store global.
        set_read_only_recursive(&dest)?;

        // meta json
        let meta = Meta {
            name: format!("{}/{}", coords.vendor, coords.package),
            version: coords.version.clone(),
            sha256: sha,
        };
        let meta_path = self.meta_path(coords);
        if let Some(parent) = meta_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&meta_path, serde_json::to_vec_pretty(&meta)?)?;

        Ok(())
    }

    /// Remove um pacote do store (dir + meta). Reabilita escrita antes (store é
    /// read-only). Caller deve segurar o lock exclusivo. No-op se ausente.
    /// Base para o GC (M5) e para reparo de entrada corrompida.
    pub fn remove_package(&self, coords: &PackageCoords) -> Result<(), StoreError> {
        let dest = self.package_path(coords);
        if dest.exists() {
            set_writable_recursive(&dest)?;
            fs::remove_dir_all(&dest)?;
        }
        let meta = self.meta_path(coords);
        if meta.exists() {
            fs::remove_file(&meta)?;
        }
        Ok(())
    }
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), StoreError> {
    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry.map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
        // symlinks dentro de pacotes são raros; M1 ignora (follow_links=false não
        // os segue e is_file()/is_dir() são falsos para eles). Tratar em M2 se surgir.
    }
    Ok(())
}

fn fsync_tree(root: &Path) -> Result<(), StoreError> {
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        if entry.file_type().is_file() {
            let f = fs::File::open(entry.path())?;
            f.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_read_only_recursive(root: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let meta = entry
            .metadata()
            .map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let mut perms = meta.permissions();
        let mode = perms.mode();
        // remove todos os bits de escrita, preserva leitura/execução
        perms.set_mode(mode & !0o222);
        fs::set_permissions(entry.path(), perms)?;
    }
    Ok(())
}

#[cfg(windows)]
fn set_read_only_recursive(root: &Path) -> Result<(), StoreError> {
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let mut perms = entry
            .metadata()
            .map_err(|e| StoreError::Io(std::io::Error::other(e)))?
            .permissions();
        perms.set_readonly(true);
        fs::set_permissions(entry.path(), perms)?;
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn set_read_only_recursive(_root: &Path) -> Result<(), StoreError> {
    Ok(()) // plataforma sem modelo de permissão conhecido — no-op
}

// Restaura só owner-write (0o200); se o pacote tinha group/other-write, esses bits não voltam.
// Aceitável no M1 (self-heal owner-driven); revisar no M3 (hard-link).
#[cfg(unix)]
fn set_writable_recursive(root: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let meta = entry
            .metadata()
            .map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let mut perms = meta.permissions();
        let mode = perms.mode();
        perms.set_mode(mode | 0o200); // owner write
        fs::set_permissions(entry.path(), perms)?;
    }
    Ok(())
}

#[cfg(windows)]
fn set_writable_recursive(root: &Path) -> Result<(), StoreError> {
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let mut perms = entry
            .metadata()
            .map_err(|e| StoreError::Io(std::io::Error::other(e)))?
            .permissions();
        perms.set_readonly(false);
        fs::set_permissions(entry.path(), perms)?;
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn set_writable_recursive(_root: &Path) -> Result<(), StoreError> {
    Ok(())
}
