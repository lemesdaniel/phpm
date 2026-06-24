use linker::materialize::{materialize_package, LinkMode};
use linker::{sync, SyncReport};
use lockfile::ComposerLock;
use std::fs;
use store::Store;
use tempfile::TempDir;

#[cfg(unix)]
fn ino(p: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(p).unwrap().ino()
}

fn fake_pkg(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("composer.json"), b"{\"name\":\"acme/pkg\"}").unwrap();
    fs::write(dir.join("src/A.php"), b"<?php class A {}").unwrap();
}

#[test]
#[cfg(unix)]
fn materialize_hardlinks_files_sharing_inodes() {
    let src = TempDir::new().unwrap();
    let dst_root = TempDir::new().unwrap();
    fake_pkg(src.path());
    let dst = dst_root.path().join("acme/pkg");

    let n = materialize_package(src.path(), &dst, LinkMode::HardLink).unwrap();
    assert_eq!(n, 2, "two files materialized");

    assert_eq!(fs::read(dst.join("composer.json")).unwrap(), b"{\"name\":\"acme/pkg\"}");
    assert_eq!(fs::read(dst.join("src/A.php")).unwrap(), b"<?php class A {}");
    assert_eq!(ino(&src.path().join("src/A.php")), ino(&dst.join("src/A.php")));
}

#[test]
#[cfg(unix)]
fn materialize_is_idempotent_skips_correct_links() {
    let src = TempDir::new().unwrap();
    let dst_root = TempDir::new().unwrap();
    fake_pkg(src.path());
    let dst = dst_root.path().join("acme/pkg");

    let first = materialize_package(src.path(), &dst, LinkMode::HardLink).unwrap();
    assert_eq!(first, 2);
    let second = materialize_package(src.path(), &dst, LinkMode::HardLink).unwrap();
    assert_eq!(second, 0, "already-correct links are skipped");
}

#[test]
#[cfg(unix)]
fn materialize_relinks_when_target_has_different_inode() {
    let src = TempDir::new().unwrap();
    let dst_root = TempDir::new().unwrap();
    fake_pkg(src.path());
    let dst = dst_root.path().join("acme/pkg");
    // pre-existing target with different content/inode (simulates an upgrade)
    fs::create_dir_all(dst.join("src")).unwrap();
    fs::write(dst.join("src/A.php"), b"OLD").unwrap();
    fs::write(dst.join("composer.json"), b"OLD").unwrap();

    let n = materialize_package(src.path(), &dst, LinkMode::HardLink).unwrap();
    assert_eq!(n, 2, "both files re-linked");
    assert_eq!(ino(&src.path().join("src/A.php")), ino(&dst.join("src/A.php")));
    assert_eq!(fs::read(dst.join("src/A.php")).unwrap(), b"<?php class A {}");
}

#[test]
fn materialize_copy_mode_duplicates_content() {
    let src = TempDir::new().unwrap();
    let dst_root = TempDir::new().unwrap();
    fake_pkg(src.path());
    let dst = dst_root.path().join("acme/pkg");

    let n = materialize_package(src.path(), &dst, LinkMode::Copy).unwrap();
    assert_eq!(n, 2);
    assert_eq!(fs::read(dst.join("src/A.php")).unwrap(), b"<?php class A {}");
}

#[test]
fn sync_on_empty_lock_creates_vendor_and_reports_zero() {
    let store_dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    let lock = ComposerLock {
        content_hash: "h0".into(),
        packages: vec![],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };

    let report: SyncReport = sync(project.path(), &lock, &store).unwrap();
    assert_eq!(report.materialized, 0);
    assert_eq!(report.removed, 0);
    assert!(project.path().join("vendor").is_dir());
}
