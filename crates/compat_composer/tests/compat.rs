use compat_composer::GenError;
use compat_composer::aggregate::{aggregate_autoload, AutoloadData, PathBase};
use compat_composer::classmap::{classmap_for_package, scan_php_classes};
use compat_composer::php_emit::{render_psr4_php, render_files_php, render_classmap_php};
use lockfile::{Autoload, ComposerJson};
use std::collections::BTreeMap;
use std::fs;

#[test]
fn gen_error_is_constructible() {
    let e = GenError::Io(std::io::Error::other("x"));
    assert!(format!("{e}").contains("I/O"));
}

fn psr4(map: &[(&str, &str)]) -> BTreeMap<String, Vec<String>> {
    map.iter().map(|(k, v)| (k.to_string(), vec![v.to_string()])).collect()
}

#[test]
fn aggregates_root_and_dependency_psr4_with_correct_base() {
    let root = ComposerJson {
        name: "acme/app".into(),
        autoload: Autoload { psr4: psr4(&[("App\\", "app/")]), ..Default::default() },
        ..Default::default()
    };
    let dep = ComposerJson {
        name: "monolog/monolog".into(),
        autoload: Autoload { psr4: psr4(&[("Monolog\\", "src/Monolog")]), ..Default::default() },
        ..Default::default()
    };

    let mut data = AutoloadData::default();
    aggregate_autoload(&mut data, &root, PathBase::Base, None);
    aggregate_autoload(&mut data, &dep, PathBase::Vendor, Some("monolog/monolog"));

    assert_eq!(data.psr4.get("App\\").unwrap(), &vec![PathBase::Base.join("app")]);
    assert_eq!(
        data.psr4.get("Monolog\\").unwrap(),
        &vec![PathBase::Vendor.join("monolog/monolog/src/Monolog")]
    );
}

#[test]
fn aggregates_files_with_dependency_prefix() {
    let dep = ComposerJson {
        name: "acme/helpers".into(),
        autoload: Autoload { files: vec!["src/helpers.php".into()], ..Default::default() },
        ..Default::default()
    };
    let mut data = AutoloadData::default();
    aggregate_autoload(&mut data, &dep, PathBase::Vendor, Some("acme/helpers"));
    assert_eq!(data.files, vec![PathBase::Vendor.join("acme/helpers/src/helpers.php")]);
}

#[test]
fn path_base_join_handles_empty_and_slashes() {
    assert_eq!(PathBase::Vendor.join(""), "$vendorDir");
    assert_eq!(PathBase::Base.join("/app/"), "$baseDir/app");
    assert_eq!(PathBase::Vendor.join("a/b"), "$vendorDir/a/b");
}

#[test]
fn psr4_php_is_valid_array_with_base_vars() {
    let mut psr4: BTreeMap<String, Vec<String>> = BTreeMap::new();
    psr4.insert("App\\".into(), vec![PathBase::Base.join("app")]);
    psr4.insert("Monolog\\".into(), vec![PathBase::Vendor.join("monolog/monolog/src/Monolog")]);

    let php = render_psr4_php(&psr4);
    assert!(php.starts_with("<?php"));
    assert!(php.contains("$vendorDir = dirname(__DIR__);"));
    assert!(php.contains("$baseDir = dirname($vendorDir);"));
    assert!(php.contains("'App\\\\' => array($baseDir . '/app'),"));
    assert!(php.contains("'Monolog\\\\' => array($vendorDir . '/monolog/monolog/src/Monolog'),"));
}

#[test]
fn files_php_keys_by_stable_identifier() {
    let files = vec![PathBase::Vendor.join("acme/helpers/src/helpers.php")];
    let php = render_files_php(&files);
    assert!(php.starts_with("<?php"));
    assert!(php.contains("=> $vendorDir . '/acme/helpers/src/helpers.php',"));
}

#[test]
fn classmap_php_maps_class_to_path() {
    let mut cm: BTreeMap<String, String> = BTreeMap::new();
    cm.insert("Acme\\Greet\\Hello".into(), PathBase::Vendor.join("acme/greet/src/Hello.php"));
    let php = render_classmap_php(&cm);
    assert!(php.contains("'Acme\\\\Greet\\\\Hello' => $vendorDir . '/acme/greet/src/Hello.php',"));
}

#[test]
fn scan_finds_namespaced_classes_interfaces_traits_enums() {
    let dir = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/Logger.php"),
        b"<?php\nnamespace Acme\\Log;\nclass Logger {}\ninterface Sink {}\n",
    ).unwrap();
    fs::write(
        dir.path().join("src/Level.php"),
        b"<?php\nnamespace Acme\\Log;\nenum Level: string { case Info = 'info'; }\ntrait Helper {}\n",
    ).unwrap();
    fs::write(dir.path().join("README.md"), b"# nope").unwrap();

    let found = scan_php_classes(dir.path()).unwrap();
    let mut names: Vec<&String> = found.keys().collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            &"Acme\\Log\\Helper".to_string(),
            &"Acme\\Log\\Level".to_string(),
            &"Acme\\Log\\Logger".to_string(),
            &"Acme\\Log\\Sink".to_string(),
        ]
    );
    assert_eq!(found.get("Acme\\Log\\Logger").unwrap(), "src/Logger.php");
}

#[test]
fn scan_handles_global_namespace() {
    let dir = tempfile::TempDir::new().unwrap();
    fs::write(dir.path().join("Top.php"), b"<?php\nclass Top {}\n").unwrap();
    let found = scan_php_classes(dir.path()).unwrap();
    assert_eq!(found.get("Top").unwrap(), "Top.php");
}

#[test]
fn classmap_cache_keeps_full_version_and_round_trips() {
    use store::{PackageCoords, Store};
    let store_dir = tempfile::TempDir::new().unwrap();
    let pkg_dir = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    fs::create_dir_all(pkg_dir.path().join("src")).unwrap();
    fs::write(pkg_dir.path().join("src/Hello.php"), b"<?php\nnamespace Acme;\nclass Hello {}\n").unwrap();
    let coords = PackageCoords { vendor: "acme".into(), package: "greet".into(), version: "3.8.1".into() };

    let first = classmap_for_package(&store, &coords, pkg_dir.path()).unwrap();
    // value is rebased under $vendorDir/<vendor>/<package>/<rel>
    assert_eq!(first.get("Acme\\Hello").unwrap(), "$vendorDir/acme/greet/src/Hello.php");

    // cache file exists with the FULL version in its name (footgun guard)
    let meta_parent = store.meta_path(&coords).parent().unwrap().to_path_buf();
    assert!(meta_parent.join("3.8.1.classmap.json").exists(), "cache keeps full version");

    // second call returns the same result (from cache)
    let second = classmap_for_package(&store, &coords, pkg_dir.path()).unwrap();
    assert_eq!(first, second);
}
