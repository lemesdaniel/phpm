use gc::collect::{execute_gc, plan_gc, GcPlan};
use gc::registry::Registry;
use store::{PackageCoords, Store};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn seed(store: &Store, v: &str, p: &str, ver: &str) {
    let src = TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("src")).unwrap();
    fs::write(src.path().join("composer.json"), b"{}").unwrap();
    fs::write(src.path().join("src/x.php"), b"<?php").unwrap();
    store
        .write_package(
            &PackageCoords { vendor: v.into(), package: p.into(), version: ver.into() },
            src.path(),
        )
        .unwrap();
}

fn write_lock(dir: &Path, pkgs: &[(&str, &str)]) {
    let entries: Vec<String> = pkgs
        .iter()
        .map(|(name, ver)| format!(r#"{{"name":"{name}","version":"{ver}"}}"#))
        .collect();
    let lock = format!(
        r#"{{"content-hash":"h","packages":[{}],"packages-dev":[]}}"#,
        entries.join(",")
    );
    fs::write(dir.join("composer.lock"), lock).unwrap();
}

#[test]
fn plan_gc_marks_unreferenced_for_removal() {
    let store_dir = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed(&store, "monolog", "monolog", "3.8.1");
    seed(&store, "old", "lib", "1.0.0");

    let proj = TempDir::new().unwrap();
    write_lock(proj.path(), &[("monolog/monolog", "3.8.1")]);

    let plan: GcPlan =
        plan_gc(&store, &[proj.path().to_str().unwrap().to_string()]).unwrap();
    assert_eq!(plan.to_remove.len(), 1);
    assert_eq!(plan.to_remove[0].vendor, "old");
    assert_eq!(plan.referenced_count, 1);
}

#[test]
fn execute_gc_removes_only_unreferenced() {
    let store_dir = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed(&store, "monolog", "monolog", "3.8.1");
    seed(&store, "old", "lib", "1.0.0");
    let proj = TempDir::new().unwrap();
    write_lock(proj.path(), &[("monolog/monolog", "3.8.1")]);

    let plan = plan_gc(&store, &[proj.path().to_str().unwrap().to_string()]).unwrap();
    let removed = execute_gc(&store, &plan).unwrap();
    assert_eq!(removed, 1);
    assert!(store.has(&PackageCoords {
        vendor: "monolog".into(),
        package: "monolog".into(),
        version: "3.8.1".into()
    }));
    assert!(!store.has(&PackageCoords {
        vendor: "old".into(),
        package: "lib".into(),
        version: "1.0.0".into()
    }));
}

#[test]
fn plan_gc_ignores_missing_lock() {
    let store_dir = TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    seed(&store, "a", "b", "1.0.0");
    // project dir with NO composer.lock → contributes no references; package becomes removable
    let proj = TempDir::new().unwrap();
    let plan = plan_gc(&store, &[proj.path().to_str().unwrap().to_string()]).unwrap();
    assert_eq!(plan.to_remove.len(), 1);
    assert_eq!(plan.referenced_count, 0);
}

#[test]
fn registry_registers_and_lists_unique_projects() {
    let home = TempDir::new().unwrap();
    let reg = Registry::new(home.path());
    reg.register("/home/me/app-a").unwrap();
    reg.register("/home/me/app-b").unwrap();
    reg.register("/home/me/app-a").unwrap(); // dedup
    let mut got = reg.list().unwrap();
    got.sort();
    assert_eq!(got, vec!["/home/me/app-a".to_string(), "/home/me/app-b".to_string()]);
}

#[test]
fn registry_prune_drops_missing_paths() {
    let home = TempDir::new().unwrap();
    let existing = TempDir::new().unwrap();
    let reg = Registry::new(home.path());
    reg.register(existing.path().to_str().unwrap()).unwrap();
    reg.register("/no/such/path/xyz").unwrap();
    reg.prune_missing().unwrap();
    let got = reg.list().unwrap();
    assert_eq!(got, vec![existing.path().to_str().unwrap().to_string()]);
}

#[test]
fn registry_empty_when_never_registered() {
    let home = TempDir::new().unwrap();
    let reg = Registry::new(home.path());
    assert!(reg.list().unwrap().is_empty());
}
