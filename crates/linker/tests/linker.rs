use linker::materialize::{materialize_package, LinkMode};
use linker::reconcile::current_vendor_packages;
use linker::sentinel::{read_sentinel, write_sentinel};
use linker::{sync, SyncReport};
use lockfile::ComposerLock;
use std::fs;
use store::{PackageCoords, Store};
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

#[test]
fn sentinel_round_trips_content_hash() {
    let project = TempDir::new().unwrap();
    let vendor = project.path().join("vendor");
    std::fs::create_dir_all(&vendor).unwrap();

    assert_eq!(read_sentinel(&vendor).unwrap(), None, "absent sentinel → None");

    write_sentinel(&vendor, "abc123").unwrap();
    assert_eq!(read_sentinel(&vendor).unwrap().as_deref(), Some("abc123"));
}

#[test]
fn sentinel_absent_when_vendor_missing() {
    let project = TempDir::new().unwrap();
    let vendor = project.path().join("vendor"); // not created
    assert_eq!(read_sentinel(&vendor).unwrap(), None);
}

#[test]
fn sentinel_corrupt_file_reads_as_none() {
    let project = TempDir::new().unwrap();
    let vendor = project.path().join("vendor");
    std::fs::create_dir_all(&vendor).unwrap();
    // garbage / non-JSON content → treated as absent so sync re-reconciles instead of erroring
    std::fs::write(vendor.join(".phpm-state"), b"not json {{{").unwrap();
    assert_eq!(read_sentinel(&vendor).unwrap(), None);
}

#[test]
fn current_vendor_packages_lists_materialized_dirs() {
    let project = TempDir::new().unwrap();
    let vendor = project.path().join("vendor");
    std::fs::create_dir_all(vendor.join("acme/pkg")).unwrap();
    std::fs::create_dir_all(vendor.join("monolog/monolog")).unwrap();
    // non-package noise that must be ignored
    std::fs::create_dir_all(vendor.join("composer")).unwrap();
    std::fs::create_dir_all(vendor.join("bin")).unwrap();
    std::fs::write(vendor.join(".phpm-state"), b"{}").unwrap();
    std::fs::write(vendor.join("autoload.php"), b"<?php").unwrap();

    let mut got = current_vendor_packages(&vendor).unwrap();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("acme".to_string(), "pkg".to_string()),
            ("monolog".to_string(), "monolog".to_string()),
        ]
    );
}

#[test]
fn current_vendor_packages_empty_when_no_vendor() {
    let project = TempDir::new().unwrap();
    let vendor = project.path().join("vendor");
    assert!(current_vendor_packages(&vendor).unwrap().is_empty());
}

/// Put a package into the store via the real store API, so sync can link from it.
fn seed_store(store: &Store, vendor: &str, package: &str, version: &str) {
    let src = TempDir::new().unwrap();
    std::fs::create_dir_all(src.path().join("src")).unwrap();
    std::fs::write(
        src.path().join("composer.json"),
        format!("{{\"name\":\"{vendor}/{package}\"}}"),
    )
    .unwrap();
    std::fs::write(src.path().join("src/A.php"), b"<?php class A {}").unwrap();
    let coords = PackageCoords {
        vendor: vendor.into(),
        package: package.into(),
        version: version.into(),
    };
    store.write_package(&coords, src.path()).unwrap();
}

fn pkg(name: &str, version: &str) -> lockfile::LockedPackage {
    lockfile::LockedPackage {
        name: name.into(),
        version: version.into(),
        package_type: "library".into(),
        dist: None,
        source: None,
    }
}

#[test]
#[cfg(unix)]
fn sync_materializes_lock_packages_from_store() {
    let store_dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed_store(&store, "acme", "pkg", "1.0.0");

    let lock = ComposerLock {
        content_hash: "h1".into(),
        packages: vec![pkg("acme/pkg", "1.0.0")],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };

    let report = sync(project.path(), &lock, &store).unwrap();
    assert_eq!(report.materialized, 1);
    assert!(!report.no_op);

    let vp = project.path().join("vendor/acme/pkg");
    assert_eq!(std::fs::read(vp.join("src/A.php")).unwrap(), b"<?php class A {}");
    assert_eq!(
        linker::sentinel::read_sentinel(&project.path().join("vendor"))
            .unwrap()
            .as_deref(),
        Some("h1")
    );
}

#[test]
#[cfg(unix)]
fn sync_is_no_op_when_sentinel_matches() {
    let store_dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed_store(&store, "acme", "pkg", "1.0.0");
    let lock = ComposerLock {
        content_hash: "h1".into(),
        packages: vec![pkg("acme/pkg", "1.0.0")],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };
    sync(project.path(), &lock, &store).unwrap();
    let report = sync(project.path(), &lock, &store).unwrap();
    assert!(report.no_op);
    assert_eq!(report.materialized, 0);
}

#[test]
#[cfg(unix)]
fn sync_removes_stale_packages_not_in_lock() {
    let store_dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed_store(&store, "acme", "pkg", "1.0.0");
    seed_store(&store, "old", "gone", "1.0.0");

    let lock_a = ComposerLock {
        content_hash: "hA".into(),
        packages: vec![pkg("acme/pkg", "1.0.0"), pkg("old/gone", "1.0.0")],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };
    sync(project.path(), &lock_a, &store).unwrap();
    assert!(project.path().join("vendor/old/gone").exists());

    let lock_b = ComposerLock {
        content_hash: "hB".into(),
        packages: vec![pkg("acme/pkg", "1.0.0")],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };
    let report = sync(project.path(), &lock_b, &store).unwrap();
    assert_eq!(report.removed, 1);
    assert!(!project.path().join("vendor/old/gone").exists());
    assert!(project.path().join("vendor/acme/pkg").exists());
}

#[test]
#[cfg(unix)]
fn sync_includes_dev_packages() {
    let store_dir = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed_store(&store, "phpunit", "phpunit", "11.0.0");
    let lock = ComposerLock {
        content_hash: "hd".into(),
        packages: vec![],
        packages_dev: vec![pkg("phpunit/phpunit", "11.0.0")],
        plugin_api_version: String::new(),
    };
    let report = sync(project.path(), &lock, &store).unwrap();
    assert_eq!(report.materialized, 1);
    assert!(project.path().join("vendor/phpunit/phpunit").exists());
}
