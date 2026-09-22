//! Integration tests for the `gwsim` binary (T0.3.4).

use assert_cmd::Command;

/// `gwsim --version` prints the workspace version.
#[test]
fn version_is_printed() {
    let expected = format!(
        "gwsim {}
",
        env!("CARGO_PKG_VERSION")
    );

    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .arg("--version")
        .assert()
        .success()
        .stdout(expected);
}

/// An unrecognised subcommand is a usage error, not a silent success.
#[test]
fn unknown_subcommand_fails() {
    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .arg("definitely-not-a-command")
        .assert()
        .failure();
}
