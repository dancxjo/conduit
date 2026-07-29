use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const CLI_FIXTURE: &str = include_str!("../../../conformance/c3/conduct-cli-v1.json");

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduct"));
    for variable in ["NO_COLOR", "CLICOLOR", "CLICOLOR_FORCE", "TERM", "COLUMNS"] {
        command.env_remove(variable);
    }
    command
}

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello.panel")
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

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn fixture_input(case: &serde_json::Value) -> Option<Vec<u8>> {
    if let Some(value) = case.get("stdin").and_then(serde_json::Value::as_str) {
        return Some(match value {
            "$HELLO_SOURCE" => include_bytes!("../../../examples/hello.panel").to_vec(),
            value => value.as_bytes().to_vec(),
        });
    }
    case.get("stdin_hex")
        .and_then(serde_json::Value::as_str)
        .map(decode_hex)
}

fn fixture_snapshot(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/snapshots")
            .join(name),
    )
    .unwrap()
}

fn assert_fixture_stream(
    case_id: &str,
    expected: &serde_json::Value,
    stream_name: &str,
    actual: &[u8],
) {
    if let Some(value) = expected
        .get(stream_name)
        .and_then(serde_json::Value::as_str)
    {
        if value == "empty" {
            assert!(actual.is_empty(), "{case_id}: {stream_name}");
        } else {
            assert_eq!(actual, value.as_bytes(), "{case_id}: {stream_name}");
        }
    }
    if let Some(prefix) = expected
        .get(format!("{stream_name}_starts"))
        .and_then(serde_json::Value::as_str)
    {
        assert!(
            actual.starts_with(prefix.as_bytes()),
            "{case_id}: {stream_name}"
        );
    }
    if let Some(fragment) = expected
        .get(format!("{stream_name}_contains"))
        .and_then(serde_json::Value::as_str)
    {
        assert!(
            String::from_utf8_lossy(actual).contains(fragment),
            "{case_id}: {stream_name}"
        );
    }
    if let Some(snapshot) = expected
        .get(format!("{stream_name}_snapshot"))
        .and_then(serde_json::Value::as_str)
    {
        assert_eq!(
            actual,
            fixture_snapshot(snapshot),
            "{case_id}: {stream_name}"
        );
    }
}

#[test]
fn every_process_conformance_vector_executes() {
    let fixture: serde_json::Value = serde_json::from_str(CLI_FIXTURE).unwrap();
    let cases = fixture["command_cases"].as_array().unwrap();
    let mut delegated_runners = Vec::new();

    for case in cases {
        let case_id = case["id"].as_str().unwrap();
        if case["runner"] != "process" {
            delegated_runners.push((case_id, case["runner"].as_str().unwrap()));
            continue;
        }

        let arguments: Vec<String> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| match value.as_str().unwrap() {
                "$EXAMPLE" => example().to_string_lossy().into_owned(),
                value => value.to_owned(),
            })
            .collect();
        let mut process = command();
        process.args(&arguments);
        for (name, value) in case
            .get("environment")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flatten()
        {
            process.env(name, value.as_str().unwrap());
        }
        let output = if let Some(input) = fixture_input(case) {
            let mut child = process
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&input).unwrap();
            child.wait_with_output().unwrap()
        } else {
            process.stdin(Stdio::null()).output().unwrap()
        };

        let expected = &case["expected"];
        assert_eq!(
            output.status.code(),
            expected["exit"].as_i64().map(|value| value as i32),
            "{case_id}: exit"
        );
        assert_fixture_stream(case_id, expected, "stdout", &output.stdout);
        assert_fixture_stream(case_id, expected, "stderr", &output.stderr);
        if let Some(code) = expected
            .get("stderr_code")
            .and_then(serde_json::Value::as_str)
        {
            let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(diagnostic["code"], code, "{case_id}: stderr code");
        }
        if let Some(ansi) = expected.get("ansi").and_then(serde_json::Value::as_bool) {
            assert_eq!(output.stderr.contains(&0x1b), ansi, "{case_id}: ANSI");
        }
    }

    assert_eq!(
        delegated_runners,
        [
            ("broken-stdout-pipe", "broken-pipe"),
            ("closed-stderr", "closed-stderr"),
            ("stdout-write-failure", "output-failure"),
            ("warning-presentation", "diagnostic-fixture-link"),
        ]
    );
}

#[test]
fn every_canonical_invocation_preserves_modes_stdin_and_streams() {
    let example = example();
    let example = example.to_str().unwrap();

    let default_run = command().arg(example).output().unwrap();
    assert!(default_run.status.success());
    assert_eq!(default_run.stdout, b"HELLO FROM CONDUIT.\n");
    assert!(default_run.stderr.is_empty());

    let explicit_run = command().args(["--run", example]).output().unwrap();
    assert_eq!(explicit_run.stdout, default_run.stdout);
    assert!(explicit_run.stderr.is_empty());

    let checked = command().args(["--check", example]).output().unwrap();
    assert!(checked.status.success());
    assert_eq!(
        checked.stdout,
        b"ok: panel v1; 0 definitions; 3 root nodes; 2 root cords\n"
    );
    assert!(checked.stderr.is_empty());

    let explained = command().args(["--explain", example]).output().unwrap();
    assert!(explained.status.success());
    assert!(explained.stdout.starts_with(b"logical panel v1:"));
    assert!(explained.stderr.is_empty());

    let source = include_bytes!("../../../examples/hello.panel");
    for arguments in [&[][..], &["-"][..]] {
        let stdin_run = output_with_stdin(arguments, source);
        assert!(stdin_run.status.success());
        assert_eq!(stdin_run.stdout, b"HELLO FROM CONDUIT.\n");
        assert!(stdin_run.stderr.is_empty());
    }
}

#[test]
fn help_version_and_conflict_snapshots_are_exact_at_representative_widths() {
    let expected_help = include_bytes!("snapshots/help.txt");
    for width in ["40", "80", "120"] {
        let help = command()
            .arg("--help")
            .env("COLUMNS", width)
            .output()
            .unwrap();
        assert!(help.status.success(), "{width}");
        assert_eq!(help.stdout, expected_help, "{width}");
        assert!(help.stderr.is_empty(), "{width}");
    }

    let version = command().arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap(),
        format!("conduct {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(version.stderr.is_empty());

    let conflict = command().args(["--check", "--run"]).output().unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    assert!(conflict.stdout.is_empty());
    assert_eq!(
        conflict.stderr,
        include_bytes!("snapshots/argument-conflict.txt")
    );
}

#[test]
fn parser_and_argument_failures_support_lossless_json_with_clean_stdout() {
    let parser = output_with_stdin(
        &["--check", "--diagnostic-format=json", "-"],
        b"panel 1\ncord a.out b.in\n",
    );
    assert!(!parser.status.success());
    assert!(parser.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&parser.stderr).unwrap();
    assert_eq!(diagnostic["schema_version"], 1);
    assert_eq!(diagnostic["code"], "CND-SRC-001");
    assert_eq!(diagnostic["primary"]["document_id"], "stdin");
    assert!(
        diagnostic["fixes"]
            .as_array()
            .is_some_and(|fixes| !fixes.is_empty())
    );

    let arguments = command()
        .args([
            "--diagnostic-format=json",
            "--verbose-diagnostics",
            "--check",
            "--run",
        ])
        .output()
        .unwrap();
    assert_eq!(arguments.status.code(), Some(2));
    assert!(arguments.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&arguments.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-CLI-001");
    assert!(diagnostic["message"].as_str().unwrap().contains("--check"));
    assert!(
        diagnostic["notes"][0]
            .as_str()
            .unwrap()
            .starts_with("Usage:")
    );
}

#[test]
fn color_environment_and_explicit_precedence_are_stable() {
    let invalid = ["--check", "-"];
    let plain = output_with_stdin(&invalid, b"");
    assert!(!plain.stderr.contains(&0x1b));

    let no_color = command()
        .args(invalid)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!no_color.stderr.contains(&0x1b));

    let dumb = command()
        .args(invalid)
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!dumb.stderr.contains(&0x1b));

    let forced = command()
        .args(invalid)
        .env("CLICOLOR_FORCE", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(forced.stderr.starts_with(b"\x1b[1;31merror"));

    let explicit_always = command()
        .args(["--check", "--color=always", "-"])
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(explicit_always.stderr.starts_with(b"\x1b[1;31merror"));

    let explicit_never = command()
        .args(["--check", "--color=never", "-"])
        .env("CLICOLOR_FORCE", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!explicit_never.stderr.contains(&0x1b));
}

#[test]
fn non_tty_stderr_has_no_status_cursor_or_color_leakage() {
    let output = command()
        .args(["--check", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.contains(&0x1b));
    assert!(!output.stderr.contains(&b'\r'));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Checking"));
}

#[test]
fn missing_file_and_invalid_utf8_are_structured_io_diagnostics() {
    let missing = command()
        .args([
            "--check",
            "--diagnostic-format=json",
            "fixture/definitely-missing.panel",
        ])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-IO-001");

    let invalid = output_with_stdin(&["--check", "--diagnostic-format=json", "-"], &[0xff, 0xfe]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&invalid.stderr).unwrap();
    assert_eq!(diagnostic["code"], "CND-IO-001");
}

#[test]
fn broken_stdout_pipe_is_success_and_closed_stderr_is_calm() {
    let mut broken = command()
        .args(["--explain", example().to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(broken.stdout.take());
    assert!(broken.wait().unwrap().success());

    let mut closed = command()
        .args(["--check", "--run"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(closed.stderr.take());
    assert_eq!(closed.wait().unwrap().code(), Some(2));
}

#[cfg(unix)]
#[test]
fn non_broken_stdout_write_failure_is_structured() {
    let full = File::options().write(true).open("/dev/full").unwrap();
    let output = command()
        .args(["--check", example().to_str().unwrap()])
        .stdout(Stdio::from(full))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("CND-IO-002"));
}
