use linker::{sync, SyncReport};
use lockfile::ComposerLock;
use store::Store;
use tempfile::TempDir;

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
