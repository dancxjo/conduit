use std::io::Write as _;
use std::process::{Command, Stdio};

#[test]
fn parser_failures_support_lossless_json_output() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .args(["--check", "--diagnostic-format=json", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"panel 1\ncord a.out b.in\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let diagnostic: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(diagnostic["schema_version"], 1);
    assert_eq!(diagnostic["code"], "CND-SRC-001");
    assert_eq!(diagnostic["primary"]["document_id"], "stdin");
    assert!(
        diagnostic["fixes"]
            .as_array()
            .is_some_and(|fixes| !fixes.is_empty())
    );
}

#[test]
fn terminal_color_choice_is_explicit_and_stable() {
    let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .args(["--check", "--color=always", "-"])
        .stdin(Stdio::piped())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stderr.starts_with(b"\x1b[1;31merror[CND-SRC-001]"));
}
