use lockfile::parse_lock;

const MINIMAL: &str = include_str!("fixtures/minimal.lock.json");

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
    assert_eq!(source.url, "https://github.com/php-fig/log.git");
}
