use cli::install::{install, InstallOpts};
use composer_bridge::{BridgeError, Runner};
use acquire::{AcquireError, Fetcher};
use store::{PackageCoords, Store};
use std::cell::RefCell;
use std::fs;
use std::path::Path;

#[derive(Default)]
struct RecordingRunner { calls: RefCell<Vec<Vec<String>>> }
impl Runner for RecordingRunner {
    fn run(&self, _program: &str, args: &[&str], _cwd: &Path) -> Result<(), BridgeError> {
        self.calls.borrow_mut().push(args.iter().map(|s| s.to_string()).collect());
        Ok(())
    }
}

struct NoFetch;
impl Fetcher for NoFetch {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, AcquireError> { panic!("must not download") }
}

fn seed_pkg(store: &Store, project: &Path, vendor: &str, package: &str, version: &str) {
    let cj = format!("{{\"name\":\"{vendor}/{package}\",\"autoload\":{{\"psr-4\":{{\"Acme\\\\\":\"src/\"}}}}}}");
    let src = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("src")).unwrap();
    fs::write(src.path().join("composer.json"), &cj).unwrap();
    fs::write(src.path().join("src/X.php"), b"<?php\nnamespace Acme;\nclass X {}\n").unwrap();
    store.write_package(&PackageCoords { vendor: vendor.into(), package: package.into(), version: version.into() }, src.path()).unwrap();
    let dest = project.join(format!("vendor/{vendor}/{package}"));
    fs::create_dir_all(dest.join("src")).unwrap();
    fs::write(dest.join("composer.json"), &cj).unwrap();
    fs::write(dest.join("src/X.php"), b"<?php\nnamespace Acme;\nclass X {}\n").unwrap();
}

#[test]
fn install_end_to_end_offline() {
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let registry_home = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());

    seed_pkg(&store, project.path(), "acme", "greet", "1.0.0");
    fs::write(project.path().join("composer.json"),
        br#"{"name":"acme/app","scripts":{"post-autoload-dump":["@php -r \"echo 1;\""]}}"#).unwrap();
    fs::write(project.path().join("composer.lock"),
        br#"{"content-hash":"h1","packages":[{"name":"acme/greet","version":"1.0.0"}],"packages-dev":[]}"#).unwrap();

    let runner = RecordingRunner::default();
    let opts = InstallOpts { registry_base: registry_home.path().to_path_buf() };
    install(project.path(), &store, &NoFetch, &runner, &opts).unwrap();

    assert!(project.path().join("vendor/acme/greet/src/X.php").exists());
    assert!(project.path().join("vendor/autoload.php").exists());
    assert!(project.path().join("vendor/composer/installed.json").exists());
    assert!(runner.calls.borrow().iter().any(|a| a.contains(&"run-script".to_string()) && a.contains(&"post-autoload-dump".to_string())));
    let reg = gc::registry::Registry::new(registry_home.path());
    assert_eq!(reg.list().unwrap(), vec![project.path().to_str().unwrap().to_string()]);
}
