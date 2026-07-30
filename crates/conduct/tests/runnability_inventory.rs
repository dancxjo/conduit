use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Inventory {
    schema: String,
    states: Vec<String>,
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    path: String,
    state: String,
    profile: String,
    proof: String,
    expected_stdout: Option<String>,
    expected_diagnostic: Option<String>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_panels(root: &Path, relative: &Path, found: &mut BTreeSet<String>) {
    let directory = root.join(relative);
    for entry in fs::read_dir(&directory).expect("panel inventory directory exists") {
        let entry = entry.expect("panel inventory entry is readable");
        let path = entry.path();
        if path.is_dir() {
            collect_panels(
                root,
                path.strip_prefix(root).expect("path remains under root"),
                found,
            );
        } else if path
            .extension()
            .is_some_and(|extension| extension == "panel")
        {
            found.insert(
                path.strip_prefix(root)
                    .expect("path remains under root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

fn invoke(root: &Path, entry: &Entry) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduct"));
    command.current_dir(root);
    if entry.proof == "canonical-check-rejection" {
        command.arg("--check");
    }
    command.arg(&entry.path).output().expect("conduct executes")
}

#[test]
fn every_checked_panel_has_one_verified_runnability_state() {
    let root = root();
    let inventory: Inventory = serde_json::from_str(
        &fs::read_to_string(root.join("examples/runnability-v1.json"))
            .expect("runnability inventory exists"),
    )
    .expect("runnability inventory is valid");
    assert_eq!(inventory.schema, "conduit.panel-runnability/v1");
    assert_eq!(
        inventory.states,
        ["runnable", "contract-only", "illustrative/unavailable"]
    );

    let mut checked_in = BTreeSet::new();
    for directory in ["examples", "fixtures", "conformance"] {
        collect_panels(&root, Path::new(directory), &mut checked_in);
    }
    let declared = inventory
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        declared.len(),
        inventory.entries.len(),
        "each panel has exactly one runnability declaration"
    );
    assert_eq!(declared, checked_in, "panel inventory has no gaps or drift");

    for entry in &inventory.entries {
        assert!(
            ["deterministic", "browser", "hosted", "embedded", "device"]
                .contains(&entry.profile.as_str()),
            "{} has an explicit host profile",
            entry.path
        );
        let output = invoke(&root, entry);
        let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        if entry.state == "runnable" {
            assert!(output.status.success(), "{} must run", entry.path);
            assert_eq!(
                stdout,
                entry.expected_stdout.as_deref().expect("runnable stdout"),
                "{} exact stdout",
                entry.path
            );
            assert!(stderr.is_empty(), "{} has clean stderr", entry.path);
        } else {
            assert!(!output.status.success(), "{} must fail closed", entry.path);
            assert!(
                stderr.contains(
                    entry
                        .expected_diagnostic
                        .as_deref()
                        .expect("non-runnable diagnostic")
                ),
                "{} emits its structured diagnostic: {stderr}",
                entry.path
            );
            assert!(stdout.is_empty(), "{} has clean stdout", entry.path);
        }
    }

    let lessons: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("tour/lessons/v1.json")).expect("Tour lessons exist"),
    )
    .expect("Tour lessons are valid");
    for lesson in lessons["lessons"].as_array().expect("lessons are listed") {
        if lesson["runnability"]["state"] != "runnable" {
            continue;
        }
        let id = lesson["id"].as_str().expect("lesson id");
        let path = std::env::temp_dir().join(format!(
            "conduit-tour-export-{}-{}.panel",
            std::process::id(),
            id.replace(['.', '/'], "-")
        ));
        fs::write(&path, lesson["source"].as_str().expect("lesson source"))
            .expect("exported lesson can be written");
        let checked = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg("--check")
            .arg(&path)
            .output()
            .expect("canonical check executes");
        assert!(checked.status.success(), "{id} exported source checks");
        let ran = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(&path)
            .output()
            .expect("canonical default run executes");
        fs::remove_file(&path).expect("temporary export is removed");
        assert!(ran.status.success(), "{id} exported source runs");
        assert_eq!(
            String::from_utf8(ran.stdout).expect("lesson stdout is UTF-8"),
            lesson["expected_stdout"].as_str().expect("runnable stdout"),
            "{id} exported source has the same canonical result"
        );
        assert!(
            ran.stderr.is_empty(),
            "{id} exported source has clean stderr"
        );
    }
}
