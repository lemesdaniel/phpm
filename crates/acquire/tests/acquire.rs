use acquire::{AcquireError, Fetcher};
use std::io::Write;

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
