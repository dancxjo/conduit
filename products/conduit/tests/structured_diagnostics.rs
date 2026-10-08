use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn plot_path(label: &str, source: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must follow the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-diagnostic-{label}-{}-{nonce}.conduit",
        std::process::id()
    ));
    fs::write(&path, source).expect("diagnostic fixture should be writable");
    path
}

fn diagnose(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduit"));
    command.args(["check", path.to_str().expect("UTF-8 fixture path")]);
    if json {
        command.arg("--json");
    }
    command.output().expect("diagnostic command should launch")
}

#[test]
fn malformed_plot_human_and_json_render_the_same_owned_diagnostic() {
    let path = plot_path("malformed", "not a plot\n");
    let human = diagnose(&path, false);
    let machine = diagnose(&path, true);
    assert!(!human.status.success());
    assert!(!machine.status.success());

    let human_stdout = String::from_utf8(human.stdout).unwrap();
    let machine_stdout = String::from_utf8(machine.stdout).unwrap();
    let diagnostics: Value = serde_json::from_str(&machine_stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["schema_version"], 1);
    assert_eq!(diagnostic["code"], "CND-FRM-019");
    assert!(human_stdout.contains(diagnostic["code"].as_str().unwrap()));
    assert!(human_stdout.contains(diagnostic["summary"].as_str().unwrap()));
    assert!(!machine_stdout.contains(path.to_str().unwrap()));

    fs::remove_file(path).expect("diagnostic fixture should be removable");
}

#[test]
fn unsupported_kind_keeps_source_identity_and_exact_primary_span() {
    let path = plot_path("unsupported", "plot demo {\n  x: missing/kind\n}\n");
    let output = diagnose(&path, true);
    assert!(!output.status.success());
    let diagnostics: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "CND-FRM-028");
    assert_eq!(diagnostic["source_document_id"].as_str().unwrap().len(), 64);
    assert_eq!(diagnostic["content_hash"].as_str().unwrap().len(), 64);
    assert!(diagnostic["primary_span"]["end"].as_u64().unwrap() > 0);

    fs::remove_file(path).expect("diagnostic fixture should be removable");
}

#[test]
fn quoted_ipa_refusal_preserves_the_authored_escape_in_human_and_json_diagnostics() {
    let source =
        include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit")
            .replace("ˈt͡ʃãː.n̩", r"t͡ʃ\n");
    let path = plot_path("ipa-escape", &source);
    let human = diagnose(&path, false);
    let machine = diagnose(&path, true);
    assert!(!human.status.success());
    assert!(!machine.status.success());
    let diagnostics: Value = serde_json::from_slice(&machine.stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "CND-SPC-IPA");
    let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
    let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], r"\n");
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(diagnostic["summary"].as_str().unwrap()));
    assert!(human.contains("CND-SPC-IPA"));
    fs::remove_file(path).unwrap();
}
