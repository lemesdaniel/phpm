use acquire::{AcquireError, Fetcher};
use lockfile::{Dist, Source};
use std::io::Write;
use std::process::Command;
use store::{PackageCoords, Store};

/// Fetcher de teste que devolve bytes fixos, sem rede.
struct StaticFetcher {
    bytes: Vec<u8>,
}

impl Fetcher for StaticFetcher {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, AcquireError> {
        Ok(self.bytes.clone())
    }
}

#[test]
fn static_fetcher_returns_bytes() {
    let f = StaticFetcher { bytes: vec![1, 2, 3] };
    assert_eq!(f.fetch("http://x").unwrap(), vec![1, 2, 3]);
}

/// Monta um zip em memória no formato Packagist: tudo sob um único dir-raiz.
fn make_composer_zip() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let w = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(w);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.add_directory("acme-pkg-abc123/", opts).unwrap();
        zip.start_file("acme-pkg-abc123/composer.json", opts).unwrap();
        zip.write_all(b"{\"name\":\"acme/pkg\"}").unwrap();
        zip.add_directory("acme-pkg-abc123/src/", opts).unwrap();
        zip.start_file("acme-pkg-abc123/src/A.php", opts).unwrap();
        zip.write_all(b"<?php class A {}").unwrap();
        zip.finish().unwrap();
    }
    buf
}

#[test]
fn extract_zip_strips_single_root_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bytes = make_composer_zip();
    acquire::zipx::extract_strip_root(&bytes, tmp.path()).unwrap();

    assert_eq!(
        std::fs::read(tmp.path().join("composer.json")).unwrap(),
        b"{\"name\":\"acme/pkg\"}"
    );
    assert_eq!(
        std::fs::read(tmp.path().join("src/A.php")).unwrap(),
        b"<?php class A {}"
    );
    assert!(!tmp.path().join("acme-pkg-abc123").exists());
}

#[test]
fn extract_rejects_zip_slip() {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("pkg-root/../../evil.php", opts).unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
    }
    let tmp = tempfile::TempDir::new().unwrap();
    let err = acquire::zipx::extract_strip_root(&buf, tmp.path()).unwrap_err();
    assert!(matches!(err, acquire::AcquireError::Zip(_)));
}

#[test]
fn extract_rejects_symlink_entry() {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.add_directory("pkg-root/", opts).unwrap();
        // alvo do symlink como conteúdo (formato zip de symlink unix)
        zip.add_symlink("pkg-root/link", "../../secret", opts).unwrap();
        zip.finish().unwrap();
    }
    let tmp = tempfile::TempDir::new().unwrap();
    let err = acquire::zipx::extract_strip_root(&buf, tmp.path()).unwrap_err();
    assert!(matches!(err, acquire::AcquireError::Zip(_)));
}

#[test]
fn shasum_ok_when_matches_and_skips_when_empty() {
    let bytes = b"hello world";
    // sha1("hello world") = 2aae6c35c94fcfb415dbe95f408b9ce91ee846ed
    let sha = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
    acquire::shasum::verify_sha1(bytes, sha).unwrap();
    // shasum vazio → pula (Ok)
    acquire::shasum::verify_sha1(bytes, "").unwrap();
}

#[test]
fn shasum_err_on_mismatch() {
    let err = acquire::shasum::verify_sha1(b"hello world", "0000").unwrap_err();
    assert!(matches!(err, acquire::AcquireError::Shasum { .. }));
}

#[test]
fn acquire_dist_writes_package_to_store() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let coords = PackageCoords {
        vendor: "acme".into(),
        package: "pkg".into(),
        version: "1.0.0".into(),
    };
    let dist = Dist {
        dist_type: "zip".into(),
        url: Some("http://example/acme-pkg.zip".into()),
        reference: "abc123".into(),
        shasum: String::new(),
    };
    let fetcher = StaticFetcher { bytes: make_composer_zip() };

    acquire::dist::acquire_dist(&store, &fetcher, &coords, &dist).unwrap();

    assert!(store.has(&coords));
    store.verify(&coords).unwrap();
    let composer = store.package_path(&coords).join("composer.json");
    assert_eq!(std::fs::read(&composer).unwrap(), b"{\"name\":\"acme/pkg\"}");
}

#[test]
#[ignore = "rede: rode com --ignored quando quiser validar download real"]
fn http_fetcher_downloads_real_dist() {
    use acquire::HttpFetcher;
    use acquire::Fetcher;
    let url = "https://api.github.com/repos/php-fig/log/zipball/79dff0b268932c640297f5208d6298f71855c03e";
    let fetcher = HttpFetcher::new().unwrap();
    let bytes = fetcher.fetch(url).unwrap();
    assert!(bytes.len() > 1000, "deve baixar um zip não-trivial");
    assert_eq!(&bytes[0..2], b"PK");
}

#[test]
fn acquire_dist_errors_without_url() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let coords = PackageCoords { vendor: "acme".into(), package: "pkg".into(), version: "1.0.0".into() };
    let dist = Dist { dist_type: "zip".into(), url: None, reference: "r".into(), shasum: String::new() };
    let fetcher = StaticFetcher { bytes: vec![] };
    let err = acquire::dist::acquire_dist(&store, &fetcher, &coords, &dist).unwrap_err();
    assert!(matches!(err, acquire::AcquireError::NoSource(_)));
}

#[test]
fn extract_without_common_root_is_flat() {
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("a/x.php", opts).unwrap();
        zip.write_all(b"a").unwrap();
        zip.start_file("b/y.php", opts).unwrap();
        zip.write_all(b"b").unwrap();
        zip.finish().unwrap();
    }
    let tmp = tempfile::TempDir::new().unwrap();
    acquire::zipx::extract_strip_root(&buf, tmp.path()).unwrap();
    assert!(tmp.path().join("a/x.php").exists());
    assert!(tmp.path().join("b/y.php").exists());
}

/// Cria um repositório git local, commita 1 arquivo, devolve (dir, sha).
fn make_git_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(ok.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&ok.stderr));
    };
    run(&["init", "-q"]);
    std::fs::write(dir.path().join("composer.json"), b"{\"name\":\"acme/git\"}").unwrap();
    run(&["add", "."]);
    run(&["-c", "commit.gpgsign=false", "commit", "-qm", "init"]);
    let out = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(dir.path()).output().unwrap();
    let sha = String::from_utf8(out.stdout).unwrap().trim().to_string();
    (dir, sha)
}

#[test]
fn acquire_git_source_writes_package() {
    let (repo, sha) = make_git_repo();
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let coords = PackageCoords { vendor: "acme".into(), package: "git".into(), version: "1.0.0".into() };
    let source = Source {
        source_type: "git".into(),
        url: Some(format!("file://{}", repo.path().display())),
        reference: sha,
    };

    acquire::git::acquire_git(&store, &coords, &source).unwrap();

    assert!(store.has(&coords));
    store.verify(&coords).unwrap();
    assert!(store.package_path(&coords).join("composer.json").exists());
    assert!(!store.package_path(&coords).join(".git").exists());
}

#[test]
fn acquire_git_errors_without_url() {
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let coords = PackageCoords { vendor: "acme".into(), package: "git".into(), version: "1.0.0".into() };
    let source = Source { source_type: "git".into(), url: None, reference: "r".into() };
    let err = acquire::git::acquire_git(&store, &coords, &source).unwrap_err();
    assert!(matches!(err, acquire::AcquireError::NoSource(_)));
}

#[test]
fn acquire_git_empty_reference_uses_head() {
    let (repo, _sha) = make_git_repo();
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let coords = PackageCoords { vendor: "acme".into(), package: "githead".into(), version: "1.0.0".into() };
    let source = Source { source_type: "git".into(), url: Some(format!("file://{}", repo.path().display())), reference: String::new() };
    acquire::git::acquire_git(&store, &coords, &source).unwrap();
    assert!(store.has(&coords));
}

#[test]
fn acquire_git_rejects_dash_reference() {
    let (repo, _sha) = make_git_repo();
    let tmp = tempfile::TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let coords = PackageCoords { vendor: "acme".into(), package: "dash".into(), version: "1.0.0".into() };
    let source = Source {
        source_type: "git".into(),
        url: Some(format!("file://{}", repo.path().display())),
        reference: "--upload-pack=evil".into(),
    };
    let err = acquire::git::acquire_git(&store, &coords, &source).unwrap_err();
    assert!(matches!(err, acquire::AcquireError::Git(_)));
}
