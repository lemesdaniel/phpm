use lockfile::parse_lock;

const MINIMAL: &str = include_str!("fixtures/minimal.lock.json");

#[test]
fn parses_content_hash() {
    let lock = parse_lock(MINIMAL).expect("deve parsear");
    assert_eq!(lock.content_hash, "a1b2c3d4e5f6");
}
