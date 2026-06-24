use std::fs;
use store::{sha256_tree, PackageCoords, Store};
use tempfile::TempDir;

/// Cria um diretório-fonte fake (simula pacote já extraído) e devolve o TempDir.
fn fake_source() -> TempDir {
    let src = TempDir::new().unwrap();
    fs::create_dir_all(src.path().join("src")).unwrap();
    fs::write(src.path().join("src/Logger.php"), b"<?php class Logger {}").unwrap();
    fs::write(src.path().join("composer.json"), b"{\"name\":\"monolog/monolog\"}").unwrap();
    src
}

#[test]
fn write_package_materializes_tree_and_meta() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let src = fake_source();

    assert!(!store.has(&coords()));
    store.write_package(&coords(), src.path()).unwrap();
    assert!(store.has(&coords()));

    // conteúdo presente
    let logger = store.package_path(&coords()).join("src/Logger.php");
    assert_eq!(fs::read(&logger).unwrap(), b"<?php class Logger {}");

    // meta json escrito com sha256
    let meta_raw = fs::read_to_string(store.meta_path(&coords())).unwrap();
    assert!(meta_raw.contains("\"sha256\""));
    assert!(meta_raw.contains("monolog/monolog"));

    let meta: serde_json::Value = serde_json::from_str(&meta_raw).unwrap();
    let sha = meta["sha256"].as_str().unwrap();
    assert_eq!(sha.len(), 64, "sha256 deve ter 64 chars hex");
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn write_package_reheals_orphaned_dir_without_meta() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    // simula crash: dir do pacote existe mas sem meta
    fs::create_dir_all(store.package_path(&coords())).unwrap();
    assert!(!store.meta_path(&coords()).exists());
    // write deve auto-curar (re-materializar), não erro
    store.write_package(&coords(), fake_source().path()).unwrap();
    assert!(store.meta_path(&coords()).exists());
    let logger = store.package_path(&coords()).join("src/Logger.php");
    assert_eq!(fs::read(&logger).unwrap(), b"<?php class Logger {}");
}

#[test]
fn write_package_twice_errors_already_exists() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    store.write_package(&coords(), fake_source().path()).unwrap();
    let err = store.write_package(&coords(), fake_source().path()).unwrap_err();
    assert!(matches!(err, store::StoreError::AlreadyExists(_)));
}

#[test]
fn write_package_leaves_no_temp_on_success() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    store.write_package(&coords(), fake_source().path()).unwrap();
    // diretório de temporários do store deve estar vazio
    let tmp_dir = tmp.path().join("tmp");
    if tmp_dir.exists() {
        assert_eq!(fs::read_dir(&tmp_dir).unwrap().count(), 0);
    }
}

fn coords() -> PackageCoords {
    PackageCoords {
        vendor: "monolog".into(),
        package: "monolog".into(),
        version: "3.8.1".into(),
    }
}

#[test]
fn verify_passes_for_intact_package() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    store.write_package(&coords(), fake_source().path()).unwrap();
    assert!(store.verify(&coords()).unwrap(), "pacote íntegro deve verificar");
}

#[test]
fn verify_fails_when_meta_missing() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    store.write_package(&coords(), fake_source().path()).unwrap();
    fs::remove_file(store.meta_path(&coords())).unwrap();
    let err = store.verify(&coords()).unwrap_err();
    assert!(matches!(err, store::StoreError::MissingMeta(_)));
}

#[test]
fn verify_fails_on_integrity_mismatch() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    store.write_package(&coords(), fake_source().path()).unwrap();
    // adultera o sha gravado no meta (meta é writable)
    let meta_path = store.meta_path(&coords());
    let tampered = r#"{"name":"monolog/monolog","version":"3.8.1","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#;
    fs::write(&meta_path, tampered).unwrap();
    let err = store.verify(&coords()).unwrap_err();
    assert!(matches!(err, store::StoreError::Integrity { .. }));
}

#[test]
fn package_path_follows_layout() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let p = store.package_path(&coords());
    assert!(p.ends_with("packages/monolog/monolog/3.8.1"));
}

#[test]
fn has_is_false_for_missing_package() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    assert!(!store.has(&coords()));
}

#[test]
fn has_is_true_when_package_dir_exists() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let c = coords();
    std::fs::create_dir_all(store.package_path(&c)).unwrap();
    assert!(store.has(&c));
}

#[test]
fn from_name_rejects_malformed() {
    assert!(PackageCoords::from_name("symfony/http-kernel/extra", "1.0").is_none());
    assert!(PackageCoords::from_name("/pkg", "1.0").is_none());
    assert!(PackageCoords::from_name("vendor/", "1.0").is_none());
}

#[test]
fn coords_from_composer_name_splits_on_slash() {
    let c = PackageCoords::from_name("monolog/monolog", "3.8.1").unwrap();
    assert_eq!(c.vendor, "monolog");
    assert_eq!(c.package, "monolog");
    assert_eq!(c.version, "3.8.1");
    // nome de plataforma sem barra → None
    assert!(PackageCoords::from_name("php", "8.2").is_none());
}

#[test]
fn meta_path_preserves_full_version() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    let p = store.meta_path(&coords());
    // Must end with the full version "3.8.1.json", NOT "3.8.json"
    assert!(
        p.ends_with("meta/monolog/monolog/3.8.1.json"),
        "meta_path was: {}",
        p.display()
    );
}

#[test]
fn tree_hash_is_stable_and_order_independent() {
    let a = TempDir::new().unwrap();
    fs::create_dir_all(a.path().join("src")).unwrap();
    fs::write(a.path().join("src/Logger.php"), b"<?php class Logger {}").unwrap();
    fs::write(a.path().join("composer.json"), b"{}").unwrap();

    let b = TempDir::new().unwrap();
    // mesmos arquivos, criados em ordem inversa
    fs::write(b.path().join("composer.json"), b"{}").unwrap();
    fs::create_dir_all(b.path().join("src")).unwrap();
    fs::write(b.path().join("src/Logger.php"), b"<?php class Logger {}").unwrap();

    assert_eq!(sha256_tree(a.path()).unwrap(), sha256_tree(b.path()).unwrap());
}

#[test]
fn tree_hash_changes_with_content() {
    let a = TempDir::new().unwrap();
    fs::write(a.path().join("f.php"), b"one").unwrap();
    let h1 = sha256_tree(a.path()).unwrap();
    fs::write(a.path().join("f.php"), b"two").unwrap();
    let h2 = sha256_tree(a.path()).unwrap();
    assert_ne!(h1, h2);
}

#[test]
fn tree_hash_changes_with_path() {
    let a = TempDir::new().unwrap();
    fs::write(a.path().join("a.php"), b"x").unwrap();
    let h1 = sha256_tree(a.path()).unwrap();

    let b = TempDir::new().unwrap();
    fs::write(b.path().join("b.php"), b"x").unwrap();
    let h2 = sha256_tree(b.path()).unwrap();
    // mesmo conteúdo, nome diferente → hash diferente
    assert_ne!(h1, h2);
}

#[test]
fn tree_hash_empty_dir_is_stable() {
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();
    assert_eq!(sha256_tree(a.path()).unwrap(), sha256_tree(b.path()).unwrap());
}

#[test]
#[cfg(unix)]
fn stored_files_are_read_only() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    store.write_package(&coords(), fake_source().path()).unwrap();

    let logger = store.package_path(&coords()).join("src/Logger.php");
    let mode = fs::metadata(&logger).unwrap().permissions().mode();
    // nenhum bit de escrita (owner/group/other)
    assert_eq!(mode & 0o222, 0, "arquivo do store deve ser read-only, mode={:o}", mode);

    // escrita deve falhar
    let write_result = fs::OpenOptions::new().write(true).open(&logger);
    assert!(write_result.is_err(), "escrita em arquivo do store deveria falhar");

    let src_dir = store.package_path(&coords()).join("src");
    let dmode = fs::metadata(&src_dir).unwrap().permissions().mode();
    assert_eq!(dmode & 0o222, 0, "diretório do store deve ser read-only, mode={:o}", dmode);
}

#[test]
#[cfg(unix)]
fn write_package_reheals_readonly_orphan() {
    let tmp = TempDir::new().unwrap();
    let store = Store::new(tmp.path());
    // primeira escrita completa → dir read-only
    store.write_package(&coords(), fake_source().path()).unwrap();
    // simula crash pós-read-only: remove só o meta, deixando dir read-only sem meta
    fs::remove_file(store.meta_path(&coords())).unwrap();
    // segunda escrita deve auto-curar mesmo com dir read-only
    store.write_package(&coords(), fake_source().path()).unwrap();
    assert!(store.meta_path(&coords()).exists());

    use std::os::unix::fs::PermissionsExt;
    let logger = store.package_path(&coords()).join("src/Logger.php");
    let mode = fs::metadata(&logger).unwrap().permissions().mode();
    assert_eq!(mode & 0o222, 0, "pacote re-curado deve voltar read-only");
}
