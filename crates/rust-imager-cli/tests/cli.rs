//! CLI behavior tests.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn version_reports_package_version() {
    Command::cargo_bin("rust-imager")
        .expect("binary should build")
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}
