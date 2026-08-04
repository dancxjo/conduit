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
    assert!(lines.len() >= 18, "unexpected output: {stdout}");
    assert_eq!(lines.first().copied(), Some("signal 0 off"));
    assert_eq!(lines.get(1).copied(), Some("signal 1 on"));
    assert_eq!(lines.get(15).copied(), Some("signal 15 on"));
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
