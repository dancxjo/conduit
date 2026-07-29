use std::fs::File;
use std::io::{BufRead, BufReader, Write as _};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const OUTPUT_FIXTURE: &str = include_str!("../../../conformance/c3/conduct-output-v1.json");

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

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello.panel")
}

fn execute(arguments: &[String]) -> Output {
    command()
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn mode_flag(mode: &str) -> String {
    format!("--{mode}")
}

fn assert_clean_machine_stdout(stdout: &[u8]) {
    assert!(!stdout.contains(&0x1b), "ANSI leaked to stdout");
    assert!(!stdout.contains(&b'\r'), "cursor rewrite leaked to stdout");
    let text = String::from_utf8_lossy(stdout);
    for prose in ["Checking", "Resolving", "Running", "Finished", "error["] {
        assert!(!text.contains(prose), "{prose} leaked to stdout");
    }
}

#[test]
fn finite_results_and_run_records_are_versioned_structured_values() {
    let example = example().to_string_lossy().into_owned();

    let check = execute(&["--check".into(), "--format=json".into(), example.clone()]);
    assert!(check.status.success());
    assert!(check.stderr.is_empty());
    assert_clean_machine_stdout(&check.stdout);
    let check: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(check["schema"], "conduit.result/v1");
    assert_eq!(check["schema_version"], 1);
    assert_eq!(check["operation"], "check");
    assert_eq!(check["result"]["panel_version"], 1);
    assert_eq!(check["result"]["root_nodes"], 3);
    assert_eq!(check["result"]["root_cords"], 2);

    let explain = execute(&["--explain".into(), "--format=json".into(), example.clone()]);
    assert!(explain.status.success());
    assert!(explain.stderr.is_empty());
    assert_clean_machine_stdout(&explain.stdout);
    let explain: serde_json::Value = serde_json::from_slice(&explain.stdout).unwrap();
    assert_eq!(explain["schema"], "conduit.result/v1");
    assert_eq!(explain["schema_version"], 1);
    assert_eq!(explain["operation"], "explain");
    assert_eq!(explain["result"]["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(explain["result"]["cords"].as_array().unwrap().len(), 2);
    assert_eq!(explain["result"]["cords"][0]["pressure"], "block(fifo)");

    let run = execute(&["--run".into(), "--format=ndjson".into(), example]);
    assert!(run.status.success());
    assert!(run.stderr.is_empty());
    assert_clean_machine_stdout(&run.stdout);
    let records: Vec<serde_json::Value> = run
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(records.len(), 2);
    for (sequence, record) in records.iter().enumerate() {
        assert_eq!(record["schema"], "conduit.run/v1");
        assert_eq!(record["schema_version"], 1);
        assert_eq!(record["sequence"], sequence);
    }
    assert_eq!(records[0]["record"], "value");
    assert_eq!(records[0]["channel"], "stdout");
    assert_eq!(records[0]["encoding"], "hex");
    assert_eq!(
        records[0]["payload_hex"],
        "48454c4c4f2046524f4d20434f4e445549542e0a"
    );
    assert_eq!(records[1]["record"], "summary");
    assert_eq!(records[1]["nodes_completed"], 3);
    assert_eq!(records[1]["cords_conducted"], 2);

    let mut child = command()
        .args(["--run", "--format=ndjson", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"panel 1\nnode message : conduit/literal { value = \"semantic error\\n\" }\n\
              node sink : conduit/stderr\ncord message.out -> sink.in\n",
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let records: Vec<serde_json::Value> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(records[0]["record"], "value");
    assert_eq!(records[0]["channel"], "stderr");
    assert_eq!(records[0]["payload_hex"], "73656d616e746963206572726f720a");
    assert_eq!(records[1]["record"], "summary");
}

#[test]
fn every_result_and_diagnostic_format_combination_keeps_streams_separate() {
    let fixture: serde_json::Value = serde_json::from_str(OUTPUT_FIXTURE).unwrap();
    let example = example().to_string_lossy().into_owned();

    for case in fixture["format_cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let mode = case["mode"].as_str().unwrap();
        let format = case["format"].as_str().unwrap();
        let accepted = case["expected"]["accepted"].as_bool().unwrap();
        for diagnostic_format in ["human", "json"] {
            let output = execute(&[
                mode_flag(mode),
                format!("--format={format}"),
                format!("--diagnostic-format={diagnostic_format}"),
                example.clone(),
            ]);
            assert_eq!(
                output.status.success(),
                accepted,
                "{id}/{diagnostic_format}"
            );
            if accepted {
                assert!(output.stderr.is_empty(), "{id}/{diagnostic_format}");
                if format != "human" {
                    assert_clean_machine_stdout(&output.stdout);
                }
            } else {
                assert!(output.stdout.is_empty(), "{id}/{diagnostic_format}");
                if diagnostic_format == "json" {
                    let diagnostic: serde_json::Value =
                        serde_json::from_slice(&output.stderr).unwrap();
                    assert_eq!(
                        diagnostic["code"], "CND-CLI-003",
                        "{id}/{diagnostic_format}"
                    );
                } else {
                    assert!(
                        String::from_utf8_lossy(&output.stderr).contains("CND-CLI-003"),
                        "{id}/{diagnostic_format}"
                    );
                }
            }
        }
    }

    for (mode, format) in [
        ("check", "human"),
        ("check", "json"),
        ("explain", "human"),
        ("explain", "json"),
        ("run", "human"),
        ("run", "ndjson"),
    ] {
        for diagnostic_format in ["human", "json"] {
            let output = command()
                .args([
                    mode_flag(mode),
                    format!("--format={format}"),
                    format!("--diagnostic-format={diagnostic_format}"),
                    "-".into(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    child
                        .stdin
                        .take()
                        .unwrap()
                        .write_all(b"panel 1\ncord missing.out absent.in\n")?;
                    child.wait_with_output()
                })
                .unwrap();
            assert_eq!(output.status.code(), Some(2), "{mode}/{format}");
            assert!(output.stdout.is_empty(), "{mode}/{format}");
            if diagnostic_format == "json" {
                let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
                assert_eq!(diagnostic["schema_version"], 1);
                assert_eq!(diagnostic["code"], "CND-SRC-001");
            } else {
                assert!(String::from_utf8_lossy(&output.stderr).contains("CND-SRC-001"));
            }
        }
    }
}

#[test]
fn quiet_verbosity_and_malformed_options_preserve_required_output() {
    let example = example().to_string_lossy().into_owned();
    let quiet = execute(&[
        "--check".into(),
        "--format=json".into(),
        "--quiet".into(),
        example.clone(),
    ]);
    assert!(quiet.status.success());
    assert!(!quiet.stdout.is_empty());
    assert!(quiet.stderr.is_empty());

    let ci = command()
        .args(["--check", "-vv"])
        .arg(&example)
        .env("CI", "true")
        .output()
        .unwrap();
    assert!(ci.status.success());
    assert!(!ci.stdout.is_empty());
    assert!(ci.stderr.is_empty());

    for arguments in [
        vec!["--quiet", "-v", "--check", &example],
        vec!["--format=yaml", "--check", &example],
        vec!["--diagnostic-format=ndjson", "--check", &example],
    ] {
        let output = command().args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("CND-CLI-001"));
    }

    let quiet_diagnostic = command()
        .args([
            "--quiet",
            "--verbose-diagnostics",
            "--check",
            "--diagnostic-format=json",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"panel 1\ncord a.out b.in\n")?;
            child.wait_with_output()
        })
        .unwrap();
    assert_eq!(quiet_diagnostic.status.code(), Some(2));
    assert!(quiet_diagnostic.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&quiet_diagnostic.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-SRC-001");
}

#[test]
fn ndjson_pipe_closure_is_success_and_other_output_failure_is_diagnostic() {
    let example = example();
    let mut child = command()
        .args(["--run", "--format=ndjson"])
        .arg(&example)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let mut child = command()
        .args(["--run", "--format=ndjson"])
        .arg(&example)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut first_record = String::new();
    reader.read_line(&mut first_record).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&first_record).unwrap()["record"],
        "value"
    );
    drop(reader);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let full = File::options().write(true).open("/dev/full").unwrap();
    let output = command()
        .args(["--run", "--format=ndjson", "--diagnostic-format=json"])
        .arg(example)
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-IO-002");
}
