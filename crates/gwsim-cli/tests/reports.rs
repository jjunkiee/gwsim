//! T4.9.10: the evaluate, log and compare commands, their reports, and the
//! result file's shape.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use gwsim_data::DataSet;
use gwsim_data::source::DirSource;
use gwsim_data::template::SkillTemplate;
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
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("reports");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The §14.1 notes every report must carry (T4.9.3).
fn assert_notes(text: &str) {
    for needle in [
        "Absolute numbers are uncalibrated",
        "assumptions touched:",
        "draft skills used",
        "coverage limits",
        "data pack: ",
        "seeds: master ",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
    }
}

fn evaluate_to(path: &Path, extra: &[&str]) -> serde_json::Value {
    let mut args = vec![
        "evaluate",
        "--party",
        "m1-mesmerway",
        "--situation",
        "kournan-patrol-hm",
        "--runs",
        "4",
        "--json",
    ];
    let path_text = path.to_str().unwrap();
    args.push(path_text);
    args.extend_from_slice(extra);
    let output = gwsim().args(&args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_notes(&String::from_utf8_lossy(&output.stdout));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The keys of a JSON value, recursively, with array elements merged.
fn shape(value: &serde_json::Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let path = format!("{prefix}.{key}");
                out.push(path.clone());
                // Party and situation are the data's own shapes, snapshotted
                // by the data crate.
                if key != "party" && key != "situation" {
                    shape(value, &path, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter().take(1) {
                shape(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_result_file_has_the_documented_shape() {
    let json = evaluate_to(&scratch("shape.json"), &[]);
    assert_eq!(json["schema_version"], 1);
    let mut keys = Vec::new();
    shape(&json, "", &mut keys);
    keys.sort();
    keys.dedup();
    insta::assert_snapshot!(keys.join("\n"));
}

#[test]
fn report_1_codes_round_trip() {
    let json = evaluate_to(&scratch("codes.json"), &[]);
    let data = DataSet::load(&DirSource::new(root().join("data"))).unwrap();
    let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
    for (build, slot) in json["builds"].as_array().unwrap().iter().zip(&party.slots) {
        let code = build["skill_code"].as_str().unwrap();
        let decoded = SkillTemplate::decode(code).unwrap();
        assert_eq!(decoded.skills, slot.build.skills, "{}", slot.name);
        assert_eq!(decoded.primary, slot.build.primary);
        assert_eq!(decoded.encode(), code);
    }
}

#[test]
fn evaluate_prints_every_report_section() {
    let output = gwsim()
        .args([
            "evaluate",
            "--party",
            "m1-mesmerway",
            "--situation",
            "kournan-patrol-hm",
            "--runs",
            "4",
            "--breakdown",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert_notes(&text);
    for needle in [
        "Builds",
        "skill code:",
        "win rate:",
        "clear time:",
        "deaths:",
        "damage taken:",
        "energy left:",
        "Contributions per run",
        "Energy over time",
    ] {
        assert!(text.contains(needle), "missing {needle:?}");
    }
    // Report 4 credits Shelter's prevented damage to hero 7.
    assert!(
        text.lines()
            .any(|l| l.contains("hero 7") && l.contains("Shelter")),
        "{text}"
    );
}

#[test]
fn evaluate_takes_a_set_on_its_own() {
    let path = scratch("set-only.json");
    let output = gwsim()
        .args([
            "evaluate",
            "--party",
            "m1-mesmerway",
            "--set",
            "m1",
            "--runs",
            "2",
            "--json",
            path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(json["situations"].as_array().unwrap().len(), 6);
    assert!(json["set"]["weighted_win_rate"].as_f64().is_some());
    assert_eq!(json["notes"]["runs"].as_array().unwrap().len(), 6);
}

#[test]
fn evaluate_accepts_skill_codes_for_a_quick_party() {
    gwsim()
        .args([
            "evaluate",
            "--party",
            "OQBTAUBPQaJ4EY6x0JHAnKDAuE",
            "--situation",
            "dummies-hm",
            "--runs",
            "2",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 code(s)"))
        .stdout(predicate::str::contains("Energy Surge"));
}

#[test]
fn evaluate_refuses_unreviewed_skills_when_asked() {
    gwsim()
        .args([
            "evaluate",
            "--party",
            "m1-mesmerway",
            "--situation",
            "kournan-patrol-hm",
            "--runs",
            "2",
            "--reviewed-only",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not Reviewed"));
}

#[test]
fn evaluate_reports_bad_inputs() {
    gwsim()
        .args(["evaluate", "--party", "no-such-party", "--situation", "dummies-hm"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not a party file"));
    gwsim()
        .args(["evaluate", "--party", "m0-player", "--situation", "nowhere"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("no situation"));
    gwsim()
        .args(["evaluate", "--party", "m0-player"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--situation"));
}

#[test]
fn a_logged_run_reproduces_its_summary() {
    let path = scratch("log.json");
    evaluate_to(&path, &[]);
    let output = gwsim()
        .args(["log", "--result", path.to_str().unwrap(), "--run", "2"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    assert!(!text.contains("does not match"));
    assert!(text.contains("outcome "));
    assert_notes(&text);

    let jsonl = gwsim()
        .args([
            "log",
            "--result",
            path.to_str().unwrap(),
            "--run",
            "2",
            "--format",
            "jsonl",
            "--verbose",
        ])
        .output()
        .unwrap();
    assert!(jsonl.status.success());
    let lines = String::from_utf8_lossy(&jsonl.stdout);
    assert!(lines.lines().count() > 100);
    for line in lines.lines() {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }

    gwsim()
        .args(["log", "--result", path.to_str().unwrap(), "--run", "99"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("there is no run 99"));
}

#[test]
fn log_refuses_a_changed_data_pack_unless_forced() {
    let path = scratch("stale.json");
    let mut json = evaluate_to(&path, &[]);
    json["inputs"]["pack"]["content_hash"] = "0000000000000000".into();
    std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
    gwsim()
        .args(["log", "--result", path.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("--force"));
    gwsim()
        .args(["log", "--result", path.to_str().unwrap(), "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("warning: the data pack has changed"));
}

#[test]
fn log_refuses_another_schema_version() {
    let path = scratch("future.json");
    let mut json = evaluate_to(&path, &[]);
    json["schema_version"] = 99.into();
    std::fs::write(&path, serde_json::to_string(&json).unwrap()).unwrap();
    gwsim()
        .args(["log", "--result", path.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("schema version"));
}

#[test]
fn comparing_a_party_with_itself_shows_no_difference() {
    let output = gwsim()
        .args([
            "compare",
            "m1-mesmerway",
            "m1-mesmerway",
            "--situations",
            "kournan-patrol-hm",
            "--runs",
            "8",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let situation = &json["situations"][0];
    assert_eq!(situation["verdict"], "NotDifferent");
    for metric in situation["metrics"].as_array().unwrap() {
        assert_eq!(metric["difference"]["mean"], 0.0, "{metric}");
        assert_eq!(metric["difference"]["ci"]["low"], 0.0);
        assert_eq!(metric["difference"]["ci"]["high"], 0.0);
    }
}

#[test]
fn comparing_with_a_weakened_party_shows_a_significant_difference() {
    // The player alone against the full party.
    let output = gwsim()
        .args([
            "compare",
            "OQBTAUBPQaJ4EY6x0JHAnKDAuE",
            "m1-mesmerway",
            "--situations",
            "kournan-patrol-hm",
            "--runs",
            "16",
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("verdict: B (M1 Mesmerway) is better"), "{text}");
    assert_notes(&text);
}
