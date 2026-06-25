use cli::install::{install, InstallOpts};
use composer_bridge::SystemRunner;
use std::process::Command;
use store::Store;

fn have(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[ignore = "real: needs composer + php + network; run with --ignored"]
fn laravel_boots_after_phpm_install() {
    assert!(have("composer") && have("php"), "needs composer + php");

    let work = tempfile::TempDir::new().unwrap();
    let project = work.path().join("app");
    let out = Command::new("composer")
        .args([
            "create-project",
            "laravel/laravel",
            "app",
            "--no-install",
            "--no-scripts",
            "--no-interaction",
            "--prefer-dist",
        ])
        .current_dir(work.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "create-project failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let runner = SystemRunner;
    composer_bridge::update(&runner, &project).unwrap();

    let store = Store::new(work.path().join("store"));
    let fetcher = acquire::HttpFetcher::new().unwrap();
    let opts = InstallOpts {
        registry_base: work.path().join("registry"),
        no_dev: false,
    };
    install(&project, &store, &fetcher, &runner, &opts).unwrap();

    assert!(project.join("vendor/autoload.php").exists());
    assert!(
        project.join("bootstrap/cache/packages.php").exists(),
        "package:discover did not run"
    );
    let artisan = Command::new("php")
        .arg("artisan")
        .arg("--version")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        artisan.status.success(),
        "artisan failed: {}",
        String::from_utf8_lossy(&artisan.stderr)
    );
    assert!(String::from_utf8_lossy(&artisan.stdout).contains("Laravel"));
}

#[test]
#[ignore = "real: needs composer + php + network; run with --ignored"]
fn symfony_console_runs_after_phpm_install() {
    assert!(have("composer") && have("php"), "needs composer + php");
    let work = tempfile::TempDir::new().unwrap();
    let project = work.path().join("app");
    // --no-install is intentionally absent: Symfony Flex must run to scaffold bin/console
    let out = Command::new("composer")
        .args([
            "create-project",
            "symfony/skeleton",
            "app",
            "--no-interaction",
            "--prefer-dist",
        ])
        .current_dir(work.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "create-project failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let runner = SystemRunner;
    composer_bridge::update(&runner, &project).unwrap();
    let store = Store::new(work.path().join("store"));
    let fetcher = acquire::HttpFetcher::new().unwrap();
    let opts = InstallOpts {
        registry_base: work.path().join("registry"),
        no_dev: false,
    };
    install(&project, &store, &fetcher, &runner, &opts).unwrap();

    assert!(project.join("vendor/autoload.php").exists());
    let console = Command::new("php")
        .arg("bin/console")
        .arg("--version")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        console.status.success(),
        "bin/console failed: {}",
        String::from_utf8_lossy(&console.stderr)
    );
}

#[test]
#[ignore = "real: needs composer + php + network; run with --ignored"]
fn phpunit_bin_runs_after_phpm_install() {
    assert!(have("composer") && have("php"), "needs composer + php");
    let work = tempfile::TempDir::new().unwrap();
    let project = work.path().join("app");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("composer.json"),
        br#"{"name":"acme/app","require-dev":{"phpunit/phpunit":"^11.0"}}"#,
    )
    .unwrap();

    let runner = SystemRunner;
    composer_bridge::update(&runner, &project).unwrap();
    let store = Store::new(work.path().join("store"));
    let fetcher = acquire::HttpFetcher::new().unwrap();
    let opts = InstallOpts {
        registry_base: work.path().join("registry"),
        no_dev: false,
    };
    install(&project, &store, &fetcher, &runner, &opts).unwrap();

    let phpunit = Command::new("php")
        .arg("vendor/bin/phpunit")
        .arg("--version")
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        phpunit.status.success(),
        "vendor/bin/phpunit failed: {}",
        String::from_utf8_lossy(&phpunit.stderr)
    );
    assert!(String::from_utf8_lossy(&phpunit.stdout).contains("PHPUnit"));
}
