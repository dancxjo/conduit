use std::path::PathBuf;
use std::process::Command;

#[test]
fn signal_demo_runs_locally() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let form_path = workspace_root.join("examples/signal-demo.form");
    let placements_path = workspace_root.join("examples/std-local.placements");

    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            form_path.to_str().expect("form path must be utf-8"),
            "--placements",
            placements_path
                .to_str()
                .expect("placements path must be utf-8"),
        ])
        .output()
        .expect("failed to run conduit binary");

    assert!(output.status.success(), "process failed: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 22, "unexpected output: {stdout}");
    assert!(
        lines
            .first()
            .is_some_and(|line| line.starts_with("host std-host-1 boot boot-")
                && line.ends_with(" profile rust-std protocol 1")),
        "missing fresh host/boot report: {stdout}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("place pulse kind=flow/pulse host=std-host-1 boot=boot-")
                && line.ends_with(" capability=pulse-1 implementation=std/pulse-v1 artifact=conduit-signal/pulse-artifact-v1")),
        "missing pulse placement line: {stdout}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("place show kind=presentation/show host=std-host-1 boot=boot-")
                && line.ends_with(" capability=stdout-show-1 implementation=std/stdout-show-signal-v1 artifact=conduit-signal/show-artifact-v1")),
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
            .any(|line| line.starts_with("plan ") && line.ends_with(" complete")),
        "missing plan completion line: {stdout}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line == &"receipts 16 first=(0, false) last=(15, true)"),
        "missing receipt summary: {stdout}"
    );
}
