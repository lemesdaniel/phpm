use composer_bridge::{update, require, remove, BridgeError, Runner};
use std::cell::RefCell;
use std::path::Path;

#[derive(Default)]
struct RecordingRunner {
    calls: RefCell<Vec<(String, Vec<String>)>>,
}
impl Runner for RecordingRunner {
    fn run(&self, program: &str, args: &[&str], _cwd: &Path) -> Result<(), BridgeError> {
        self.calls.borrow_mut().push((program.to_string(), args.iter().map(|s| s.to_string()).collect()));
        Ok(())
    }
}

#[test]
fn update_invokes_composer_update_no_install() {
    let r = RecordingRunner::default();
    update(&r, Path::new("/tmp/proj")).unwrap();
    let calls = r.calls.borrow();
    assert_eq!(calls[0].0, "composer");
    assert_eq!(calls[0].1, vec!["update", "--no-install", "--no-interaction"]);
}

#[test]
fn require_passes_packages_and_no_install() {
    let r = RecordingRunner::default();
    require(&r, Path::new("/tmp/proj"), &["monolog/monolog:^3.0".into()]).unwrap();
    assert_eq!(r.calls.borrow()[0].1, vec!["require", "--no-install", "--no-interaction", "monolog/monolog:^3.0"]);
}

#[test]
fn remove_passes_packages_and_no_install() {
    let r = RecordingRunner::default();
    remove(&r, Path::new("/tmp/proj"), &["monolog/monolog".into()]).unwrap();
    assert_eq!(r.calls.borrow()[0].1, vec!["remove", "--no-install", "--no-interaction", "monolog/monolog"]);
}
