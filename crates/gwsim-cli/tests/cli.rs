//! Integration tests for the `gwsim` binary (T0.3.4).

use assert_cmd::Command;

/// `gwsim --version` prints the workspace version and the data pack's hash.
///
/// The hash is there because two builds of the same version can hold
/// different data, and a result is only reproducible against the data it came
/// from (ENG-43, T1.7.6).
#[test]
fn version_is_printed() {
    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::starts_with(format!(
            "gwsim {}",
            env!("CARGO_PKG_VERSION")
        )))
        .stdout(predicates::str::contains("(data "));
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

/// The repository's own data passes, and the command says so out loud.
#[test]
fn data_validate_accepts_the_repositorys_data() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");

    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .args(["data", "validate", "--data-dir"])
        .arg(&data_dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("no problems found"));
}

/// A tree that cannot be read fails with a non-zero exit code, which is what
/// makes the CI step in T1.2.12 worth having.
#[test]
fn data_validate_fails_on_a_missing_tree() {
    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .args(["data", "validate", "--data-dir", "no/such/place"])
        .assert()
        .failure()
        .stdout(predicates::str::contains("--data-dir"));
}

/// JSON output is machine-readable even when there is nothing to report.
#[test]
fn data_validate_json_is_always_an_array() {
    let data_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");

    let output = Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .args(["data", "validate", "--format", "json", "--data-dir"])
        .arg(&data_dir)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).expect("output should be UTF-8");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("output should be JSON");
    assert!(parsed.is_array(), "expected an array, got {parsed}");
}

/// `gwsim data` with no subcommand is a usage error rather than a no-op.
#[test]
fn data_without_a_subcommand_fails() {
    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .arg("data")
        .assert()
        .failure();
}

/// `gwsim template decode` on the player's published code (T1.3.6).
#[test]
fn template_decode_prints_the_players_build() {
    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .args(["template", "decode", "OQBTAUBPQaJ4EY6x0BAAAAAAuE"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Mesmer"))
        .stdout(predicates::str::contains("DominationMagic"));
}

/// A code that is not a template fails rather than printing an empty build.
#[test]
fn template_decode_rejects_rubbish() {
    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .args(["template", "decode", "not-a-real-code"])
        .assert()
        .failure()
        .stdout(predicates::str::contains("not a template code"));
}

/// Decoding and re-encoding a published code through the CLI reproduces it.
#[test]
fn template_encode_round_trips_through_a_file() {
    let code = "OQBTAUBPQaJ4EY6x0BAAAAAAuE";
    let template = gwsim_data::template::SkillTemplate::decode(code).expect("should decode");
    let text = ron::ser::to_string_pretty(&template, Default::default()).expect("serialise");

    let path = std::env::temp_dir().join("gwsim-cli-template.ron");
    std::fs::write(&path, text).expect("write the template file");

    Command::cargo_bin("gwsim")
        .expect("the gwsim binary should be built for this test")
        .args(["template", "encode"])
        .arg(&path)
        .assert()
        .success()
        .stdout(predicates::str::starts_with(code));

    std::fs::remove_file(&path).ok();
}
