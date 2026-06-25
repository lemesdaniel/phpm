use compat_composer::GenError;
use compat_composer::aggregate::{aggregate_autoload, AutoloadData, PathBase};
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
