use acquire::{AcquireError, Fetcher};
use cli::gc_run;
use cli::install::{install, InstallOpts};
use composer_bridge::{BridgeError, Runner};
use std::cell::RefCell;
use std::fs;
use std::path::Path;
use store::{PackageCoords, Store};

#[derive(Default)]
struct RecordingRunner {
    calls: RefCell<Vec<Vec<String>>>,
}
impl Runner for RecordingRunner {
    fn run(&self, _program: &str, args: &[&str], _cwd: &Path) -> Result<(), BridgeError> {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| s.to_string()).collect());
        Ok(())
    }
}

struct NoFetch;
impl Fetcher for NoFetch {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, AcquireError> {
        panic!("must not download")
    }
}

fn seed_pkg(store: &Store, project: &Path, vendor: &str, package: &str, version: &str) {
    let cj = format!(
        "{{\"name\":\"{vendor}/{package}\",\"autoload\":{{\"psr-4\":{{\"Acme\\\\\":\"src/\"}}}}}}"
    );
    let src = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("src")).unwrap();
    fs::write(src.path().join("composer.json"), &cj).unwrap();
    fs::write(
        src.path().join("src/X.php"),
        b"<?php\nnamespace Acme;\nclass X {}\n",
    )
    .unwrap();
    store
        .write_package(
            &PackageCoords {
                vendor: vendor.into(),
                package: package.into(),
                version: version.into(),
            },
            src.path(),
        )
        .unwrap();
    let dest = project.join(format!("vendor/{vendor}/{package}"));
    fs::create_dir_all(dest.join("src")).unwrap();
    fs::write(dest.join("composer.json"), &cj).unwrap();
    fs::write(
        dest.join("src/X.php"),
        b"<?php\nnamespace Acme;\nclass X {}\n",
    )
    .unwrap();
}

#[test]
fn install_end_to_end_offline() {
    let store_dir = tempfile::TempDir::new().unwrap();
    let project = tempfile::TempDir::new().unwrap();
    let registry_home = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());

    seed_pkg(&store, project.path(), "acme", "greet", "1.0.0");
    fs::write(
        project.path().join("composer.json"),
        br#"{"name":"acme/app","scripts":{"post-autoload-dump":["@php -r \"echo 1;\""]}}"#,
    )
    .unwrap();
    fs::write(project.path().join("composer.lock"),
        br#"{"content-hash":"h1","packages":[{"name":"acme/greet","version":"1.0.0"}],"packages-dev":[]}"#).unwrap();

    let runner = RecordingRunner::default();
    let opts = InstallOpts {
        registry_base: registry_home.path().to_path_buf(),
    };
    install(project.path(), &store, &NoFetch, &runner, &opts).unwrap();

    assert!(project.path().join("vendor/acme/greet/src/X.php").exists());
    assert!(project.path().join("vendor/autoload.php").exists());
    assert!(project
        .path()
        .join("vendor/composer/installed.json")
        .exists());
    assert!(runner
        .calls
        .borrow()
        .iter()
        .any(|a| a.contains(&"run-script".to_string())
            && a.contains(&"post-autoload-dump".to_string())));
    let reg = gc::registry::Registry::new(registry_home.path());
    assert_eq!(
        reg.list().unwrap(),
        vec![project.path().to_str().unwrap().to_string()]
    );
}

#[test]
fn gc_run_dry_run_then_prune() {
    let store_dir = tempfile::TempDir::new().unwrap();
    let registry_home = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());

    // a project IS registered (so gc doesn't refuse with EmptyRegistry), but its lock
    // references nothing → the seeded package is unreferenced and removable
    let proj = tempfile::TempDir::new().unwrap();
    fs::write(
        proj.path().join("composer.lock"),
        br#"{"content-hash":"h","packages":[],"packages-dev":[]}"#,
    )
    .unwrap();
    gc::registry::Registry::new(registry_home.path())
        .register(proj.path().to_str().unwrap())
        .unwrap();

    let src = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("src")).unwrap();
    fs::write(src.path().join("composer.json"), b"{}").unwrap();
    fs::write(src.path().join("src/x.php"), b"<?php").unwrap();
    store
        .write_package(
            &PackageCoords {
                vendor: "old".into(),
                package: "lib".into(),
                version: "1.0.0".into(),
            },
            src.path(),
        )
        .unwrap();

    // dry run: reports, does NOT delete
    let report = gc_run(&store, registry_home.path(), false).unwrap();
    assert_eq!(report.would_remove, 1);
    assert_eq!(report.removed, 0);
    assert!(store.has(&PackageCoords {
        vendor: "old".into(),
        package: "lib".into(),
        version: "1.0.0".into()
    }));

    // prune: deletes
    let report = gc_run(&store, registry_home.path(), true).unwrap();
    assert_eq!(report.removed, 1);
    assert!(!store.has(&PackageCoords {
        vendor: "old".into(),
        package: "lib".into(),
        version: "1.0.0".into()
    }));
}

#[test]
#[ignore = "real: needs composer + network + php; run with --ignored"]
fn phpm_install_real_psr_log() {
    let project = tempfile::TempDir::new().unwrap();
    let store_dir = tempfile::TempDir::new().unwrap();
    let registry = tempfile::TempDir::new().unwrap();
    fs::write(
        project.path().join("composer.json"),
        br#"{"name":"acme/app","require":{"psr/log":"^3.0"}}"#,
    )
    .unwrap();

    let store = Store::new(store_dir.path());
    let runner = composer_bridge::SystemRunner;
    // real composer resolves the lock (no vendor write)
    composer_bridge::update(&runner, project.path()).unwrap();
    let fetcher = acquire::HttpFetcher::new().unwrap();
    let opts = InstallOpts {
        registry_base: registry.path().to_path_buf(),
    };
    install(project.path(), &store, &fetcher, &runner, &opts).unwrap();

    assert!(project.path().join("vendor/autoload.php").exists());
    assert!(project.path().join("vendor/psr/log/src").exists());
    let script = format!(
        "require '{}/vendor/autoload.php'; echo interface_exists('Psr\\\\Log\\\\LoggerInterface') ? 'ok' : 'no';",
        project.path().display()
    );
    let out = std::process::Command::new("php")
        .arg("-r")
        .arg(&script)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), "ok");
}

#[test]
fn gc_run_empty_registry_is_noop_not_error() {
    let store_dir = tempfile::TempDir::new().unwrap();
    let registry_home = tempfile::TempDir::new().unwrap();
    let store = Store::new(store_dir.path());
    // nothing registered → gc_run must NOT error; reports 0
    let report = gc_run(&store, registry_home.path(), true).unwrap();
    assert_eq!(report.would_remove, 0);
    assert_eq!(report.removed, 0);
}
