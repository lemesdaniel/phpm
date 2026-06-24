use lockfile::parse_lock;

const MINIMAL: &str = include_str!("fixtures/minimal.lock.json");
const WITH_DEV: &str = include_str!("fixtures/with-dev.lock.json");
const NULL_URL: &str = include_str!("fixtures/null-url.lock.json");

#[test]
fn parses_content_hash() {
    let lock = parse_lock(MINIMAL).expect("deve parsear");
    assert_eq!(lock.content_hash, "a1b2c3d4e5f6");
}

#[test]
fn parses_first_package_dist_and_source() {
    let lock = parse_lock(MINIMAL).expect("deve parsear");
    assert_eq!(lock.packages.len(), 1);
    let pkg = &lock.packages[0];
    assert_eq!(pkg.name, "psr/log");
    assert_eq!(pkg.version, "3.0.0");
    assert_eq!(pkg.package_type, "library");

    let dist = pkg.dist.as_ref().expect("tem dist");
    assert_eq!(dist.dist_type, "zip");
    assert_eq!(dist.reference, "abc123");

    let source = pkg.source.as_ref().expect("tem source");
    assert_eq!(source.source_type, "git");
    assert_eq!(
        source.url.as_deref(),
        Some("https://github.com/php-fig/log.git")
    );
}

#[test]
fn parses_dev_packages_and_ignores_unknown_fields() {
    let lock = parse_lock(WITH_DEV).expect("deve parsear");
    assert_eq!(lock.packages.len(), 1);
    assert_eq!(lock.packages_dev.len(), 1);
    assert_eq!(lock.packages_dev[0].name, "phpunit/phpunit");
    // pacote sem dist/source → None, não erro
    assert!(lock.packages[0].dist.is_none());
    assert!(lock.packages[0].source.is_none());
    // type ausente → default "library"
    assert_eq!(lock.packages[0].package_type, "library");
    // plugin-api-version ausente → string vazia (default), sem erro
    assert_eq!(lock.plugin_api_version, "");
}

#[test]
fn dist_url_is_optional() {
    let lock = parse_lock(NULL_URL).expect("deve parsear mesmo com url null");
    let no_url = &lock.packages[0];
    assert!(
        no_url.dist.as_ref().unwrap().url.is_none(),
        "url null → None"
    );
    let with_url = &lock.packages[1];
    assert_eq!(
        with_url.dist.as_ref().unwrap().url.as_deref(),
        Some("https://x/y.zip")
    );
}
