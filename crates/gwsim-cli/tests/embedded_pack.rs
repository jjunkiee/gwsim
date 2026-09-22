//! T1.7.7: the binary works with no `data/` directory present.
//!
//! This is WP1.7's whole point: a release binary copied somewhere on its own
//! must still know the game. Every test here runs from an **empty temporary
//! directory**, so nothing can quietly fall back to the repository's `data/`.

use std::path::PathBuf;

use assert_cmd::Command;

/// A fresh empty directory to run in.
fn empty_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("gwsim-empty-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("should create the temporary directory");
    path
}

fn gwsim() -> Command {
    Command::cargo_bin("gwsim").expect("the gwsim binary should be built for this test")
}

#[test]
fn data_info_works_with_no_data_directory() {
    let dir = empty_dir("info");

    gwsim()
        .current_dir(&dir)
        .args(["data", "info"])
        .assert()
        .success()
        .stdout(predicates::str::contains("built into this binary"))
        .stdout(predicates::str::contains("content hash"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn data_coverage_works_with_no_data_directory() {
    let dir = empty_dir("coverage");

    gwsim()
        .current_dir(&dir)
        .args(["data", "coverage"])
        .assert()
        .success();

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn data_validate_works_with_no_data_directory() {
    let dir = empty_dir("validate");

    gwsim()
        .current_dir(&dir)
        .args(["data", "validate"])
        .assert()
        .success()
        .stdout(predicates::str::contains("built into this binary"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_embedded_data_holds_the_assumptions_register() {
    // Proof that the pack carries real contents rather than being empty: the
    // register has 34 entries and none of them come from the file system here.
    let dir = empty_dir("contents");

    let output = gwsim()
        .current_dir(&dir)
        .args(["data", "info", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).expect("output should be UTF-8");
    let value: serde_json::Value = serde_json::from_str(&text).expect("output should be JSON");

    assert_eq!(value["counts"]["assumptions"], 34);
    assert!(
        value["embedded_bytes"].as_u64().unwrap_or(0) > 10_000,
        "the pack should hold the real data, not an empty stub"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_version_carries_the_data_hash() {
    // Two builds of the same version can hold different data, so the version
    // alone does not identify a result (ENG-43).
    let dir = empty_dir("version");

    let output = gwsim()
        .current_dir(&dir)
        .arg("--version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let text = String::from_utf8(output).expect("output should be UTF-8");
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
    assert!(
        text.contains("data "),
        "the version should name the data: {text}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_named_directory_is_used_in_preference_to_the_embedded_data() {
    // Developer mode: --data-dir wins, and the report says so, because a
    // result from local edits is not a result from the shipped data.
    let dir = empty_dir("named");
    let repo_data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");

    gwsim()
        .current_dir(&dir)
        .args(["data", "info", "--data-dir"])
        .arg(&repo_data)
        .assert()
        .success()
        .stdout(predicates::str::contains("--data-dir"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn running_gwsim_does_not_create_a_user_directory() {
    // A tool that makes folders in %APPDATA% just for being run is a tool
    // that litters.
    let dir = empty_dir("nolitter");
    let user_dir = dir.join("user-files");

    gwsim()
        .current_dir(&dir)
        .args(["data", "info", "--user-dir"])
        .arg(&user_dir)
        .assert()
        .success();

    assert!(
        !user_dir.exists(),
        "reading information must not create the user directory"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_user_directory_can_be_set_by_environment() {
    let dir = empty_dir("env");
    let user_dir = dir.join("from-env");

    gwsim()
        .current_dir(&dir)
        .env("GWSIM_USER_DIR", &user_dir)
        .args(["data", "info"])
        .assert()
        .success()
        .stdout(predicates::str::contains("from-env"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_broken_named_directory_fails_rather_than_falling_back() {
    // Silently using the embedded data when the caller asked for a directory
    // would make a typo look like a successful run against the wrong data.
    let dir = empty_dir("broken");

    gwsim()
        .current_dir(&dir)
        .args(["data", "validate", "--data-dir", "no/such/place"])
        .assert()
        .failure();

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_pack_builder_rejects_a_broken_tree() {
    // Called directly rather than through a real build, as T1.7.7 asks.
    use gwsim_data::pack::DataPack;
    use gwsim_data::source::MemSource;

    let broken = MemSource::default().with("skills/mesmer/broken.ron", "(this is not a skill file");

    let problems = DataPack::from_source(&broken).expect_err("should be rejected");
    assert!(
        problems.is_fatal(),
        "a tree that does not parse must not be packable"
    );
}

#[test]
fn a_sound_tree_packs_and_unpacks_to_the_same_data() {
    use gwsim_data::pack::DataPack;

    let repo_data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");

    let packed = DataPack::from_dir(&repo_data).expect("data/ should pack");
    let bytes = DataPack::bytes_from_source(&gwsim_data::source::DirSource::new(&repo_data))
        .expect("data/ should serialise");
    let unpacked = DataPack::from_bytes(&bytes).expect("the pack should read back");

    assert_eq!(
        packed.version.content_hash, unpacked.version.content_hash,
        "packing and unpacking must not change the content hash"
    );
    assert_eq!(packed.data.counts(), unpacked.data.counts());
}
