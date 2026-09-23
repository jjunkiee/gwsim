//! T5.8.4 and T5.7.6: `gwsim optimise` end to end, exhaustive mode, and the
//! `gwsim profile` commands.

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gwsim() -> Command {
    let mut command = Command::cargo_bin("gwsim").unwrap();
    command.current_dir(root());
    command
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("optimise");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn small_run(json: &PathBuf, extra: &[&str]) -> serde_json::Value {
    let mut args = vec![
        "optimise",
        "--party",
        "m1-mesmerway",
        "--free",
        "player",
        "--situations",
        "dummies-hm",
        "--generations",
        "1",
        "--population",
        "6",
        "--stages",
        "2,4,8",
        "--seed",
        "5",
        "--quiet",
        "--json",
        json.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    let output = gwsim().args(&args).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    for needle in [
        "Ranked builds",
        "skill code",
        "Frontier",
        "Absolute numbers are uncalibrated",
        "seeds: master",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in {text}");
    }
    serde_json::from_str(&std::fs::read_to_string(json).unwrap()).unwrap()
}

#[test]
fn a_small_optimise_runs_end_to_end_and_reproduces() {
    let a = small_run(&scratch("a.json"), &[]);
    assert_eq!(a["schema_version"], 1);
    assert_eq!(a["interrupted"], false);
    assert!(!a["outcome"]["ranked"].as_array().unwrap().is_empty());
    assert!(a["ranked_builds"][0][0]["skill_code"].is_string());
    let b = small_run(&scratch("b.json"), &[]);
    assert_eq!(
        a["outcome"], b["outcome"],
        "the same seed gives the same result"
    );
}

#[test]
fn exhaustive_mode_runs_a_tiny_pool() {
    let pool = scratch("pool.ron");
    std::fs::write(
        &pool,
        r#"(slot: "player", positions: [7, 8], skills: [Slug("power-spike"), Slug("shatter-hex"), Slug("shatter-enchantment"), Slug("drain-enchantment")])"#,
    )
    .unwrap();
    let json = scratch("exhaustive.json");
    let value = small_run(
        &json,
        &["--mode", "exhaustive", "--pool", pool.to_str().unwrap()],
    );
    assert_eq!(value["outcome"]["stop"], "Exhausted");
    // C(4, 2) = 6 combinations.
    assert_eq!(value["outcome"]["candidates"], 6);
}

#[test]
fn an_oversized_pool_is_refused() {
    let pool = scratch("big.ron");
    std::fs::write(
        &pool,
        r#"(slot: "player", k: Some(3), skills: [Slug("power-spike"), Slug("shatter-hex"), Slug("shatter-enchantment"), Slug("drain-enchantment"), Slug("mistrust"), Slug("unnatural-signet")])"#,
    )
    .unwrap();
    gwsim()
        .args([
            "optimise",
            "--party",
            "m1-mesmerway",
            "--free",
            "player",
            "--situations",
            "dummies-hm",
            "--mode",
            "exhaustive",
            "--pool",
            pool.to_str().unwrap(),
            "--limit",
            "2",
            "--quiet",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("over the limit"))
        .stdout(predicate::str::contains("evolutionary"));
}

#[test]
fn optimise_validates_its_inputs() {
    gwsim()
        .args([
            "optimise",
            "--party",
            "m1-mesmerway",
            "--free",
            "nobody",
            "--situations",
            "dummies-hm",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("no such slot"));
    gwsim()
        .args([
            "optimise",
            "--party",
            "m1-mesmerway",
            "--free",
            "player",
            "--situations",
            "dummies-hm",
            "--mode",
            "exhaustive",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("--pool"));
    gwsim()
        .args([
            "optimise",
            "--party",
            "m1-mesmerway",
            "--situations",
            "dummies-hm",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--free"));
}

#[test]
fn profiles_are_created_edited_and_listed() {
    let dir = scratch("user");
    let _ = std::fs::remove_dir_all(&dir);
    let user = dir.to_str().unwrap();
    let profile = |args: &[&str]| {
        let mut all = vec!["profile", "--user-dir", user];
        all.extend_from_slice(args);
        gwsim().args(all).assert()
    };
    profile(&["new", "mine"])
        .success()
        .stdout(predicate::str::contains("created"));
    profile(&["new", "mine"])
        .failure()
        .stdout(predicate::str::contains("already exists"));
    profile(&["list"])
        .success()
        .stdout(predicate::str::contains("mine"));
    profile(&["set", "mine", "title", "asura", "3"]).success();
    profile(&["lock", "mine", "skill", "energy-surge"]).success();
    profile(&["hero", "mine", "remove", "gwen"]).success();
    profile(&["hero", "mine", "add", "nobody"]).failure();
    profile(&["path", "mine"])
        .success()
        .stdout(predicate::str::contains("mine.ron"));
    let shown = gwsim()
        .args(["profile", "--user-dir", user, "show", "mine"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&shown.stdout);
    assert!(text.contains("Asura: 3"), "{text}");
    assert!(text.contains("Only("), "{text}");
    assert!(!text.contains("gwen"), "{text}");

    // The profile then limits a search: Energy Surge is never proposed.
    let json = scratch("profiled.json");
    let value = small_run(&json, &["--profile", "mine", "--user-dir", user]);
    for candidate in value["outcome"]["ranked"].as_array().unwrap() {
        for build in candidate["builds"].as_array().unwrap() {
            let skills = build["build"]["skills"].as_array().unwrap();
            assert!(!skills.iter().any(|s| s == 39), "{build}");
        }
    }
}
