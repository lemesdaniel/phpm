use compat_composer::aggregate::{aggregate_autoload, AutoloadData, PathBase};
use compat_composer::bin_proxies::{render_bin_proxy_bat, render_bin_proxy_php};
use compat_composer::classmap::{classmap_for_package, scan_php_classes};
use compat_composer::generate;
use compat_composer::installed::{render_installed_json, render_installed_php, InstalledPackage};
use compat_composer::php_emit::{
    render_autoload_entry, render_autoload_real, render_classmap_php, render_files_php,
    render_psr4_php,
};
use compat_composer::GenError;
use lockfile::{Autoload, ComposerJson, ComposerLock, LockedPackage};
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use store::{PackageCoords, Store};

fn php_available() -> bool {
    Command::new("php")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Seed the store + materialize a tiny PSR-4 package into vendor (M3 would do this).
fn setup_greet(store: &Store, project: &std::path::Path) {
    let body = b"<?php\nnamespace Acme\\Greet;\nclass Hello { public static function hi() { return 'hi'; } }\n";
    let cj = br#"{"name":"acme/greet","autoload":{"psr-4":{"Acme\\Greet\\":"src/"}}}"#;
    // store
    let src = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("src")).unwrap();
    fs::write(src.path().join("composer.json"), cj).unwrap();
    fs::write(src.path().join("src/Hello.php"), body).unwrap();
    let coords = PackageCoords {
        vendor: "acme".into(),
        package: "greet".into(),
        version: "1.0.0".into(),
    };
    store.write_package(&coords, src.path()).unwrap();
    // materialize into vendor
    let dest = project.join("vendor/acme/greet");
    fs::create_dir_all(dest.join("src")).unwrap();
    fs::write(dest.join("composer.json"), cj).unwrap();
    fs::write(dest.join("src/Hello.php"), body).unwrap();
}

fn greet_lock() -> ComposerLock {
    ComposerLock {
        content_hash: "h1".into(),
        packages: vec![LockedPackage {
            name: "acme/greet".into(),
            version: "1.0.0".into(),
            package_type: "library".into(),
            dist: None,
            source: None,
        }],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    }
}

#[test]
fn generate_writes_all_expected_files() {
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    setup_greet(&store, project.path());

    generate(
        project.path(),
        &greet_lock(),
        &store,
        r#"{"name":"acme/app"}"#,
    )
    .unwrap();

    let vendor = project.path().join("vendor");
    for f in [
        "autoload.php",
        "composer/ClassLoader.php",
        "composer/InstalledVersions.php",
        "composer/autoload_real.php",
        "composer/autoload_psr4.php",
        "composer/autoload_namespaces.php",
        "composer/autoload_classmap.php",
        "composer/autoload_files.php",
        "composer/installed.php",
        "composer/installed.json",
    ] {
        assert!(vendor.join(f).exists(), "missing {f}");
    }
}

#[test]
fn generated_autoloader_loads_a_class() {
    if !php_available() {
        eprintln!("skipping: php not on PATH");
        return;
    }
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    setup_greet(&store, project.path());
    generate(
        project.path(),
        &greet_lock(),
        &store,
        r#"{"name":"acme/app"}"#,
    )
    .unwrap();

    let script = format!(
        "require '{}/vendor/autoload.php'; echo \\Acme\\Greet\\Hello::hi();",
        project.path().display()
    );
    let out = Command::new("php").arg("-r").arg(&script).output().unwrap();
    assert!(
        out.status.success(),
        "php failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hi");
}

#[test]
fn generated_installed_versions_works() {
    if !php_available() {
        eprintln!("skipping: php not on PATH");
        return;
    }
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    setup_greet(&store, project.path());
    generate(
        project.path(),
        &greet_lock(),
        &store,
        r#"{"name":"acme/app"}"#,
    )
    .unwrap();

    // load autoload then query InstalledVersions for the dependency
    let script = format!(
        "require '{}/vendor/autoload.php'; echo \\Composer\\InstalledVersions::getPrettyVersion('acme/greet');",
        project.path().display()
    );
    let out = Command::new("php").arg("-r").arg(&script).output().unwrap();
    assert!(
        out.status.success(),
        "php failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1.0.0");
}

#[test]
#[cfg(unix)]
fn generate_writes_executable_bin_proxy() {
    use std::os::unix::fs::PermissionsExt;
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    // a package that declares a bin
    let cj = br#"{"name":"acme/tool","bin":["bin/acmetool"],"autoload":{"psr-4":{"Acme\\Tool\\":"src/"}}}"#;
    let src = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("bin")).unwrap();
    fs::write(src.path().join("composer.json"), cj).unwrap();
    fs::write(src.path().join("bin/acmetool"), b"<?php // tool\n").unwrap();
    let coords = PackageCoords {
        vendor: "acme".into(),
        package: "tool".into(),
        version: "1.0.0".into(),
    };
    store.write_package(&coords, src.path()).unwrap();
    let dest = project.path().join("vendor/acme/tool");
    fs::create_dir_all(dest.join("bin")).unwrap();
    fs::write(dest.join("composer.json"), cj).unwrap();
    fs::write(dest.join("bin/acmetool"), b"<?php // tool\n").unwrap();

    let lock = ComposerLock {
        content_hash: "hb".into(),
        packages: vec![LockedPackage {
            name: "acme/tool".into(),
            version: "1.0.0".into(),
            package_type: "library".into(),
            dist: None,
            source: None,
        }],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };
    generate(project.path(), &lock, &store, r#"{"name":"acme/app"}"#).unwrap();

    let proxy = project.path().join("vendor/bin/acmetool");
    assert!(proxy.exists(), "bin proxy created");
    let mode = fs::metadata(&proxy).unwrap().permissions().mode();
    assert_ne!(mode & 0o111, 0, "bin proxy must be executable");
}

#[test]
fn gen_error_is_constructible() {
    let e = GenError::Io(std::io::Error::other("x"));
    assert!(format!("{e}").contains("I/O"));
}

fn psr4(map: &[(&str, &str)]) -> BTreeMap<String, Vec<String>> {
    map.iter()
        .map(|(k, v)| (k.to_string(), vec![v.to_string()]))
        .collect()
}

#[test]
fn aggregates_root_and_dependency_psr4_with_correct_base() {
    let root = ComposerJson {
        name: "acme/app".into(),
        autoload: Autoload {
            psr4: psr4(&[("App\\", "app/")]),
            ..Default::default()
        },
        ..Default::default()
    };
    let dep = ComposerJson {
        name: "monolog/monolog".into(),
        autoload: Autoload {
            psr4: psr4(&[("Monolog\\", "src/Monolog")]),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut data = AutoloadData::default();
    aggregate_autoload(&mut data, &root, PathBase::Base, None);
    aggregate_autoload(&mut data, &dep, PathBase::Vendor, Some("monolog/monolog"));

    assert_eq!(
        data.psr4.get("App\\").unwrap(),
        &vec![PathBase::Base.join("app")]
    );
    assert_eq!(
        data.psr4.get("Monolog\\").unwrap(),
        &vec![PathBase::Vendor.join("monolog/monolog/src/Monolog")]
    );
}

#[test]
fn aggregates_files_with_dependency_prefix() {
    let dep = ComposerJson {
        name: "acme/helpers".into(),
        autoload: Autoload {
            files: vec!["src/helpers.php".into()],
            ..Default::default()
        },
        ..Default::default()
    };
    let mut data = AutoloadData::default();
    aggregate_autoload(&mut data, &dep, PathBase::Vendor, Some("acme/helpers"));
    assert_eq!(
        data.files,
        vec![PathBase::Vendor.join("acme/helpers/src/helpers.php")]
    );
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
    psr4.insert(
        "Monolog\\".into(),
        vec![PathBase::Vendor.join("monolog/monolog/src/Monolog")],
    );

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
    cm.insert(
        "Acme\\Greet\\Hello".into(),
        PathBase::Vendor.join("acme/greet/src/Hello.php"),
    );
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
    )
    .unwrap();
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
fn scan_handles_chained_modifiers_and_readonly() {
    let dir = tempfile::TempDir::new().unwrap();
    fs::write(
        dir.path().join("M.php"),
        b"<?php\nnamespace Acme;\nfinal class Alpha {}\nabstract class Beta {}\nreadonly class Gamma {}\nfinal class Delta extends Base implements Iface {}\n",
    ).unwrap();
    let found = scan_php_classes(dir.path()).unwrap();
    let mut names: Vec<&String> = found.keys().collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            &"Acme\\Alpha".to_string(),
            &"Acme\\Beta".to_string(),
            &"Acme\\Delta".to_string(),
            &"Acme\\Gamma".to_string(),
        ]
    );
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
    fs::write(
        pkg_dir.path().join("src/Hello.php"),
        b"<?php\nnamespace Acme;\nclass Hello {}\n",
    )
    .unwrap();
    let coords = PackageCoords {
        vendor: "acme".into(),
        package: "greet".into(),
        version: "3.8.1".into(),
    };

    let first = classmap_for_package(&store, &coords, pkg_dir.path()).unwrap();
    // value is rebased under $vendorDir/<vendor>/<package>/<rel>
    assert_eq!(
        first.get("Acme\\Hello").unwrap(),
        "$vendorDir/acme/greet/src/Hello.php"
    );

    // cache file exists with the FULL version in its name (footgun guard)
    let meta_parent = store.meta_path(&coords).parent().unwrap().to_path_buf();
    assert!(
        meta_parent.join("3.8.1.classmap.json").exists(),
        "cache keeps full version"
    );

    // second call returns the same result (from cache)
    let second = classmap_for_package(&store, &coords, pkg_dir.path()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn autoload_real_wires_loader_with_hash() {
    let php = render_autoload_real("phpm00000000000000000000000000000");
    assert!(php.contains("class ComposerAutoloaderInitphpm00000000000000000000000000000"));
    assert!(php.contains("require __DIR__ . '/ClassLoader.php';"));
    assert!(php.contains("$loader->setPsr4("));
    assert!(php.contains("$loader->set("));
    assert!(php.contains("$loader->addClassMap("));
    assert!(php.contains("$loader->register(true);"));
    // M4 uses the dynamic form, not the static optimization
    assert!(!php.contains("autoload_static.php"));
}

#[test]
fn autoload_entry_returns_getloader() {
    let php = render_autoload_entry("phpm00000000000000000000000000000");
    assert!(php.starts_with("<?php"));
    assert!(php.contains("require_once __DIR__ . '/composer/autoload_real.php';"));
    assert!(php
        .contains("return ComposerAutoloaderInitphpm00000000000000000000000000000::getLoader();"));
}

fn pkg_row(name: &str, ver: &str) -> InstalledPackage {
    InstalledPackage {
        name: name.into(),
        version: ver.into(),
        package_type: "library".into(),
        dist_type: "zip".into(),
        dist_url: None,
        reference: "abc123".into(),
        shasum: String::new(),
        dev: false,
    }
}

#[test]
fn installed_php_dev_requirement_reflects_dev_flag() {
    let pkgs = vec![
        InstalledPackage {
            name: "monolog/monolog".into(),
            version: "3.8.1".into(),
            package_type: "library".into(),
            dist_type: "zip".into(),
            dist_url: None,
            reference: "a".into(),
            shasum: String::new(),
            dev: false,
        },
        InstalledPackage {
            name: "phpunit/phpunit".into(),
            version: "11.0.0".into(),
            package_type: "library".into(),
            dist_type: "zip".into(),
            dist_url: None,
            reference: "b".into(),
            shasum: String::new(),
            dev: true,
        },
    ];
    let php = render_installed_php("acme/app", "1.0.0", &pkgs);
    assert!(php.contains("'dev_requirement' => false,"));
    assert!(php.contains("'dev_requirement' => true,"));
}

#[test]
fn installed_json_lists_dev_package_names() {
    let pkgs = vec![
        InstalledPackage {
            name: "monolog/monolog".into(),
            version: "3.8.1".into(),
            package_type: "library".into(),
            dist_type: "zip".into(),
            dist_url: None,
            reference: "a".into(),
            shasum: String::new(),
            dev: false,
        },
        InstalledPackage {
            name: "phpunit/phpunit".into(),
            version: "11.0.0".into(),
            package_type: "library".into(),
            dist_type: "zip".into(),
            dist_url: None,
            reference: "b".into(),
            shasum: String::new(),
            dev: true,
        },
    ];
    let json = render_installed_json(&pkgs, &std::collections::BTreeMap::new());
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed["dev-package-names"],
        serde_json::json!(["phpunit/phpunit"])
    );
}

#[test]
fn installed_php_contains_root_and_versions() {
    let pkgs = vec![pkg_row("monolog/monolog", "3.8.1")];
    let php = render_installed_php("acme/app", "1.0.0+no-version-set", &pkgs);
    assert!(php.starts_with("<?php"));
    assert!(php.contains("'root' => array("));
    assert!(php.contains("'name' => 'acme/app',"));
    assert!(php.contains("'versions' => array("));
    assert!(php.contains("'monolog/monolog' => array("));
    assert!(php.contains("'pretty_version' => '3.8.1',"));
    assert!(php.contains("'version' => '3.8.1.0',")); // normalized
    assert!(php.contains("'type' => 'library',"));
}

#[test]
fn installed_json_carries_extra_for_discovery() {
    let pkgs = vec![pkg_row("acme/provider", "1.0.0")];
    let mut extras: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    extras.insert(
        "acme/provider".into(),
        serde_json::json!({ "laravel": { "providers": ["Acme\\ServiceProvider"] } }),
    );
    let json = render_installed_json(&pkgs, &extras);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["packages"][0]["name"], "acme/provider");
    assert_eq!(parsed["packages"][0]["version"], "1.0.0");
    assert_eq!(
        parsed["packages"][0]["extra"]["laravel"]["providers"][0],
        "Acme\\ServiceProvider"
    );
}

#[test]
fn bin_proxy_php_resolves_autoload_and_includes_real_binary() {
    let php = render_bin_proxy_php("phpunit/phpunit/phpunit");
    assert!(php.starts_with("#!/usr/bin/env php\n<?php"));
    assert!(php.contains("$_composer_autoload_path"));
    assert!(php.contains("include __DIR__ . '/../phpunit/phpunit/phpunit';"));
}

#[test]
fn bin_proxy_bat_calls_php() {
    let bat = render_bin_proxy_bat("phpunit");
    assert!(bat.contains("@php "));
    assert!(bat.to_uppercase().contains("PHPUNIT"));
}

#[test]
fn generate_tolerates_missing_package_composer_json() {
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    // vendor dir exists but has NO composer.json
    fs::create_dir_all(project.path().join("vendor/acme/broken/src")).unwrap();
    let lock = ComposerLock {
        content_hash: "hbroken".into(),
        packages: vec![LockedPackage {
            name: "acme/broken".into(),
            version: "1.0.0".into(),
            package_type: "library".into(),
            dist: None,
            source: None,
        }],
        packages_dev: vec![],
        plugin_api_version: String::new(),
    };
    // must NOT error; the package is still recorded in installed
    generate(project.path(), &lock, &store, r#"{"name":"acme/app"}"#).unwrap();
    let installed =
        fs::read_to_string(project.path().join("vendor/composer/installed.json")).unwrap();
    assert!(installed.contains("acme/broken"));
}
