use gc::registry::Registry;
use tempfile::TempDir;

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
