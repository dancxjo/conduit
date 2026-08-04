use std::path::PathBuf;
use std::process::Command;

#[test]
fn signal_demo_runs_locally() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");

    let output = Command::new("cargo")
        .current_dir(&workspace_root)
        .args([
            "run",
            "--quiet",
            "-p",
            "conduit",
            "--",
            "examples/signal-demo.form",
        ])
        .output()
        .expect("failed to run conduit binary");

    assert!(output.status.success(), "process failed: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 22, "unexpected output: {stdout}");
    assert_eq!(
        lines.first().copied(),
        Some("host std-host-1 boot boot-1 profile rust-std protocol 1")
    );
    assert!(
        lines
            .iter()
            .any(|line| line == &"place pulse kind=flow/pulse host=std-host-1 boot=boot-1 capability=cap-pulse-1 implementation=std/pulse-v1"),
        "missing pulse placement line: {stdout}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line == &"place show kind=display/show host=std-host-1 boot=boot-1 capability=cap-show-stdout-1 implementation=std/stdout-show-signal-v1"),
        "missing show placement line: {stdout}"
    );
    assert!(
        lines.iter().any(|line| line == &"signal 0 off"),
        "missing first signal line: {stdout}"
    );
    assert!(
        lines.iter().any(|line| line == &"signal 1 on"),
        "missing second signal line: {stdout}"
    );
    assert!(
        lines.iter().any(|line| line == &"signal 15 on"),
        "missing last signal line: {stdout}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("plan plan-signal-demo-boot-1 complete")),
        "missing plan completion line: {stdout}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line == &"receipts 16 first=(0, false) last=(15, true)"),
        "missing receipt summary: {stdout}"
    );
}
