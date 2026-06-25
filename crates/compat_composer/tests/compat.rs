use compat_composer::GenError;
use compat_composer::aggregate::{aggregate_autoload, AutoloadData, PathBase};
use compat_composer::php_emit::{render_psr4_php, render_files_php, render_classmap_php};
use lockfile::{Autoload, ComposerJson};
use std::collections::BTreeMap;

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
