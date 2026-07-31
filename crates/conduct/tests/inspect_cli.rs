use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduct"));
    for variable in [
        "NO_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
        "CI",
        "COLUMNS",
    ] {
        command.env_remove(variable);
    }
    command
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> PathBuf {
    root().join("examples/hello.panel")
}

fn output_with_stdin(arguments: &[&str], stdin: &[u8]) -> Output {
    let mut child = command()
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn temporary(extension: &str, bytes: &[u8]) -> PathBuf {
    let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "conduit-inspect-{}-{sequence}.{extension}",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn remove_temporary(path: &Path) {
    std::fs::remove_file(path).unwrap();
}

#[test]
fn inspect_panel_human_json_stdin_and_explicit_type_agree() {
    let example = example();
    let human = command().args(["inspect"]).arg(&example).output().unwrap();
    assert!(human.status.success());
    assert!(human.stderr.is_empty());
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.starts_with("panel-source v3: valid\n"));
    assert!(human.contains("identity sha256:"));
    assert!(human.contains("nodes 3"));

    let automatic = command()
        .args(["inspect", "--format=json"])
        .arg(&example)
        .output()
        .unwrap();
    let explicit = command()
        .args(["inspect", "--type=panel", "--format=json"])
        .arg(&example)
        .output()
        .unwrap();
    assert!(automatic.status.success());
    assert!(explicit.status.success());
    assert!(automatic.stderr.is_empty());
    assert!(explicit.stderr.is_empty());
    let automatic: serde_json::Value = serde_json::from_slice(&automatic.stdout).unwrap();
    let explicit: serde_json::Value = serde_json::from_slice(&explicit.stdout).unwrap();
    assert_eq!(automatic["schema"], "conduit.result/v1");
    assert_eq!(automatic["operation"], "inspect");
    assert_eq!(automatic["result"], explicit["result"]);
    assert_eq!(automatic["result"]["schema"], "conduit.inspection/v1");
    assert_eq!(automatic["result"]["kind"], "panel-source");

    let source = std::fs::read(&example).unwrap();
    let stdin = output_with_stdin(&["inspect", "--type=panel", "--format=json", "-"], &source);
    assert!(stdin.status.success());
    assert!(stdin.stderr.is_empty());
    let stdin: serde_json::Value = serde_json::from_slice(&stdin.stdout).unwrap();
    assert_eq!(stdin["result"]["identity"], automatic["result"]["identity"]);
}

#[test]
fn evidence_diagnostic_and_conformance_kinds_keep_machine_streams_clean() {
    for (path, expected_kind) in [
        (
            root().join("conformance/c2/execution-event-v1.ndjson"),
            "execution-evidence",
        ),
        (
            root().join("conformance/c3/diagnostics-v1.json"),
            "conformance-cases",
        ),
        (
            root().join("conformance/c2/execution-plan-v1.tsv"),
            "conformance-cases",
        ),
        (
            root().join("conformance/v1/manifest.json"),
            "conformance-manifest",
        ),
    ] {
        let output = command()
            .args(["inspect", "--format=json"])
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{expected_kind}");
        assert!(output.stderr.is_empty(), "{expected_kind}");
        assert!(!output.stdout.contains(&0x1b), "{expected_kind}");
        assert!(!output.stdout.contains(&b'\r'), "{expected_kind}");
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["result"]["kind"], expected_kind);
    }

    let diagnostic = br#"{"schema_version":1,"code":"CND-TST-001","severity":"error","message":"sensitive prose","arguments":[{"name":"token","value":{"disposition":"redacted","sensitivity":"secret","value_type":"fixture/token"}}]}"#;
    let path = temporary("json", diagnostic);
    let output = command()
        .args([
            "inspect",
            "--type=diagnostic",
            "--format=json",
            "--verbose-diagnostics",
        ])
        .arg(&path)
        .output()
        .unwrap();
    remove_temporary(&path);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("sensitive prose"));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["result"]["kind"], "structured-diagnostic");
    assert_eq!(result["result"]["redacted_fields"], 1);
}

#[test]
fn unresolved_selectors_and_secrets_are_reported_without_resolution_or_disclosure() {
    let source = br#"panel 3
node app : fixture/handler using ready {
    credential = secret("canary-secret-material")
}
"#;
    let path = temporary("panel", source);
    let output = command()
        .args(["inspect", "--format=json", "-vv"])
        .arg(&path)
        .output()
        .unwrap();
    remove_temporary(&path);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("canary-secret-material"));
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(result["result"]["counts"]["unresolved_selectors"], 1);
    assert_eq!(result["result"]["redacted_fields"], 1);
    assert!(
        result["result"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note
                .as_str()
                .unwrap()
                .contains("without provider resolution"))
    );
}

#[test]
fn unknown_polyglot_malformed_oversized_and_type_conflicts_fail_closed() {
    let cases: Vec<(&str, Vec<u8>, &[&str], &str)> = vec![
        (
            "wasm",
            b"\0asm\x01\0\0\0".to_vec(),
            &["inspect", "--diagnostic-format=json"],
            "CND-INSP-001",
        ),
        (
            "json",
            br#"{"suite":"fixture/v1","schema_version":1,"code":"CND-TST-001","severity":"error"}"#
                .to_vec(),
            &["inspect", "--diagnostic-format=json"],
            "CND-INSP-002",
        ),
        (
            "json",
            b"{not-json".to_vec(),
            &["inspect", "--type=diagnostic", "--diagnostic-format=json"],
            "CND-INSP-006",
        ),
        (
            "ndjson",
            b"panel 3\n".to_vec(),
            &["inspect", "--type=panel", "--diagnostic-format=json"],
            "CND-INSP-003",
        ),
        (
            "json",
            br#"{"schema":"conduit.execution-plan/v1","schema_version":1}"#.to_vec(),
            &[
                "inspect",
                "--type=execution-plan",
                "--diagnostic-format=json",
            ],
            "CND-INSP-008",
        ),
    ];
    for (extension, bytes, arguments, code) in cases {
        let path = temporary(extension, &bytes);
        let output = command().args(arguments).arg(&path).output().unwrap();
        remove_temporary(&path);
        assert_eq!(output.status.code(), Some(2), "{code}");
        assert!(output.stdout.is_empty(), "{code}");
        let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(diagnostic["code"], code);
    }

    let oversized = temporary("panel", &vec![b' '; 8 * 1024 * 1024 + 1]);
    let output = command()
        .args(["inspect", "--diagnostic-format=json"])
        .arg(&oversized)
        .output()
        .unwrap();
    remove_temporary(&oversized);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-INSP-005");
}

#[test]
fn inspection_never_executes_input_and_normal_pipe_closure_is_success() {
    let marker =
        std::env::temp_dir().join(format!("conduit-inspect-executed-{}", std::process::id()));
    let source = format!("#!/bin/sh\ntouch {}\n", marker.display());
    let path = temporary("sh", source.as_bytes());
    let output = command()
        .args(["inspect", "--diagnostic-format=json"])
        .arg(&path)
        .output()
        .unwrap();
    remove_temporary(&path);
    assert_eq!(output.status.code(), Some(2));
    assert!(!marker.exists());

    let mut child = command()
        .args(["inspect"])
        .arg(example())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn secondary_operation_is_unambiguous_and_preserves_path_escape() {
    let help = command().arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("inspect"));

    let inspect_help = command().args(["inspect", "--help"]).output().unwrap();
    assert!(inspect_help.status.success());
    let inspect_help = String::from_utf8(inspect_help.stdout).unwrap();
    assert!(inspect_help.contains("--type <TYPE>"));
    assert!(inspect_help.contains("[possible values: auto, panel, lowered-source"));

    let missing = command().arg("inspect").output().unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("CND-CLI-001"));

    let escaped = command()
        .args(["--diagnostic-format=json", "--", "inspect"])
        .output()
        .unwrap();
    assert_eq!(escaped.status.code(), Some(2));
    let diagnostic: serde_json::Value = serde_json::from_slice(&escaped.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-IO-001");

    let conflict = command()
        .args(["--check", "inspect"])
        .arg(example())
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    assert!(conflict.stdout.is_empty());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("CND-CLI-004"));

    let ndjson = command()
        .args(["inspect", "--format=ndjson"])
        .arg(example())
        .output()
        .unwrap();
    assert_eq!(ndjson.status.code(), Some(2));
    assert!(ndjson.stdout.is_empty());
    assert!(String::from_utf8_lossy(&ndjson.stderr).contains("CND-CLI-003"));
}

#[cfg(unix)]
#[test]
fn inspect_output_failure_is_structured() {
    let full = File::options().write(true).open("/dev/full").unwrap();
    let output = command()
        .args(["inspect", "--diagnostic-format=json"])
        .arg(example())
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-IO-002");
}
