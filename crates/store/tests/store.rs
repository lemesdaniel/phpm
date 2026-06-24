use std::fs;
use store::{sha256_tree, PackageCoords, Store};
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
fn has_is_true_when_package_dir_exists() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let c = coords();
    std::fs::create_dir_all(store.package_path(&c)).unwrap();
    assert!(store.has(&c));
}

#[test]
fn from_name_rejects_malformed() {
    assert!(PackageCoords::from_name("symfony/http-kernel/extra", "1.0").is_none());
    assert!(PackageCoords::from_name("/pkg", "1.0").is_none());
    assert!(PackageCoords::from_name("vendor/", "1.0").is_none());
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

#[test]
fn tree_hash_is_stable_and_order_independent() {
    let a = TempDir::new().unwrap();
    fs::create_dir_all(a.path().join("src")).unwrap();
    fs::write(a.path().join("src/Logger.php"), b"<?php class Logger {}").unwrap();
    fs::write(a.path().join("composer.json"), b"{}").unwrap();

    let b = TempDir::new().unwrap();
    // mesmos arquivos, criados em ordem inversa
    fs::write(b.path().join("composer.json"), b"{}").unwrap();
    fs::create_dir_all(b.path().join("src")).unwrap();
    fs::write(b.path().join("src/Logger.php"), b"<?php class Logger {}").unwrap();

    assert_eq!(sha256_tree(a.path()).unwrap(), sha256_tree(b.path()).unwrap());
}

#[test]
fn tree_hash_changes_with_content() {
    let a = TempDir::new().unwrap();
    fs::write(a.path().join("f.php"), b"one").unwrap();
    let h1 = sha256_tree(a.path()).unwrap();
    fs::write(a.path().join("f.php"), b"two").unwrap();
    let h2 = sha256_tree(a.path()).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn tree_hash_changes_with_path() {
    let a = TempDir::new().unwrap();
    fs::write(a.path().join("a.php"), b"x").unwrap();
    let h1 = sha256_tree(a.path()).unwrap();

    let b = TempDir::new().unwrap();
    fs::write(b.path().join("b.php"), b"x").unwrap();
    let h2 = sha256_tree(b.path()).unwrap();
    // mesmo conteúdo, nome diferente → hash diferente
    assert_ne!(h1, h2);
}

#[test]
fn tree_hash_empty_dir_is_stable() {
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();
    assert_eq!(sha256_tree(a.path()).unwrap(), sha256_tree(b.path()).unwrap());
}
