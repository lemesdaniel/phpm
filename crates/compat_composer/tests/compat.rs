use compat_composer::GenError;

#[test]
fn gen_error_is_constructible() {
    let e = GenError::Io(std::io::Error::other("x"));
    assert!(format!("{e}").contains("I/O"));
}
