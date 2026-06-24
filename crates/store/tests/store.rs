use store::{PackageCoords, Store};
use tempfile::TempDir;

fn coords() -> PackageCoords {
    PackageCoords {
        vendor: "monolog".into(),
        package: "monolog".into(),
        version: "3.8.1".into(),
    }
}

#[test]
fn package_path_follows_layout() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let p = store.package_path(&coords());
    assert!(p.ends_with("packages/monolog/monolog/3.8.1"));
}

#[test]
fn has_is_false_for_missing_package() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    assert!(!store.has(&coords()));
}

#[test]
fn coords_from_composer_name_splits_on_slash() {
    let c = PackageCoords::from_name("monolog/monolog", "3.8.1").unwrap();
    assert_eq!(c.vendor, "monolog");
    assert_eq!(c.package, "monolog");
    assert_eq!(c.version, "3.8.1");
    // nome de plataforma sem barra → None
    assert!(PackageCoords::from_name("php", "8.2").is_none());
}

#[test]
fn meta_path_preserves_full_version() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let p = store.meta_path(&coords());
    // Must end with the full version "3.8.1.json", NOT "3.8.json"
    assert!(
        p.ends_with("meta/monolog/monolog/3.8.1.json"),
        "meta_path was: {}",
        p.display()
    );
}
