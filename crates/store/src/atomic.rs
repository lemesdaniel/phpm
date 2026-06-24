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
    /// Writes an already-extracted package (in `src_dir`) to the store atomically:
    /// copies to a temporary directory inside the store, fsyncs, and moves with
    /// an atomic rename to the final destination. Then writes the meta JSON.
    /// Errors with AlreadyExists if the destination already exists (caller must check has()
    /// under lock before calling).
    pub fn write_package(&self, coords: &PackageCoords, src_dir: &Path) -> Result<(), StoreError> {
        let dest = self.package_path(coords);
        if dest.is_dir() {
            if self.meta_path(coords).exists() {
                return Err(StoreError::AlreadyExists(format!(
                    "{}/{}@{}",
                    coords.vendor, coords.package, coords.version
                )));
            }
            // dir present but no meta = partial install (crash between rename and meta write):
            // remove the orphan and re-materialize. The dir may be read-only (crash after
            // immutability was applied), so restore write access before removing.
            set_writable_recursive(&dest)?;
            fs::remove_dir_all(&dest)?;
        }

        let tmp_root = self.root_ref().join("tmp");
        fs::create_dir_all(&tmp_root)?;
        // Deterministic per-(coords) suffix; cross-process uniqueness is guaranteed by the
        // exclusive lock the caller (acquire_package) holds since M2.
        let staging = tmp_root.join(format!(
            "{}__{}__{}.staging",
            coords.vendor, coords.package, coords.version
        ));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        copy_tree(src_dir, &staging)?;
        fsync_tree(&staging)?;

        // Compute integrity BEFORE applying read-only (chmod would otherwise block the read).
        let sha = sha256_tree(&staging).map_err(StoreError::Io)?;

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&staging, &dest)?;

        // immutability: store is read-only. A write in vendor/ (same inode via
        // hard link in M3) fails loudly instead of silently corrupting the global store.
        set_read_only_recursive(&dest)?;

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

    /// Removes a package from the store (dir + meta). Re-enables write access first (store is
    /// read-only). Caller must hold the exclusive lock. No-op if absent.
    /// Foundation for GC (M5) and for repairing a corrupted entry.
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
        // symlinks inside packages are rare; M1 ignores them (follow_links=false does not
        // follow them and is_file()/is_dir() return false for them). Handle in M2 if needed.
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
        // remove all write bits, preserve read/execute
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
    Ok(()) // platform with no known permission model — no-op
}

// Restores only owner-write (0o200); if the package had group/other-write bits, they are not restored.
// Acceptable in M1 (owner-driven self-heal); revisit in M3 (hard-link).
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
