use cli::install::{install, InstallOpts};
use composer_bridge::SystemRunner;
use store::Store;
use std::process::Command;

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
