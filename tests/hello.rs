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
    let receipt_lines = lines
        .iter()
        .filter(|line| line.starts_with("receipt signal placement="))
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        receipt_lines.len(),
        16,
        "expected one machine-readable receipt per signal: {stdout}"
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
        receipt_lines
            .iter()
            .any(|line| line.ends_with(" sequence=0 level=false")),
        "missing first signal receipt: {stdout}"
    );
    assert!(
        receipt_lines
            .iter()
            .any(|line| line.ends_with(" sequence=15 level=true")),
        "missing last signal receipt: {stdout}"
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

#[test]
fn typed_multi_value_form_runs_through_the_std_kernel() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let form_path = workspace_root.join("examples/kernel-multivalue.form");

    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "kernel-multivalue",
            form_path.to_str().expect("form path must be utf-8"),
        ])
        .output()
        .expect("failed to run conduit multi-value kernel profile");

    assert!(output.status.success(), "process failed: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    let receipt_lines = stdout
        .lines()
        .filter(|line| line.starts_with("receipt tick placement="))
        .collect::<Vec<_>>();
    assert_eq!(receipt_lines.len(), 3, "unexpected receipts: {stdout}");
    assert!(stdout.contains("tick even 0"), "{stdout}");
    assert!(stdout.contains("tick even 2"), "{stdout}");
    assert!(stdout.contains("tick latest 3"), "{stdout}");
    assert!(
        stdout.contains("receipts 3 even=(0, 2) latest=(3)"),
        "{stdout}"
    );
    assert!(stdout.contains("stable_allocations=true"), "{stdout}");
    assert!(
        stdout.contains("pressure_items=1 pressure_bytes=8"),
        "{stdout}"
    );
    assert!(
        stdout.contains("input_closed=5 terminal_order_exact=true"),
        "{stdout}"
    );
    assert!(
        stdout
            .lines()
            .any(|line| line.starts_with("plan ") && line.ends_with(" complete")),
        "{stdout}"
    );
}

#[test]
fn triple_signal_form_runs_through_local_std_kernel() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let form_path = workspace_root.join("examples/triple-signal.form");
    let placements_path = workspace_root.join("examples/triple-local.placements");

    let source = std::fs::read_to_string(&form_path).expect("triple form exists");
    for platform_fact in [
        "stdout",
        "DOM",
        "GPIO",
        "browser",
        "Pico",
        "WebSocket",
        "TCP",
        "UDP",
    ] {
        assert!(
            !source.contains(platform_fact),
            "triple form should not contain platform fact {platform_fact}"
        );
    }

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
    let receipt_lines = stdout
        .lines()
        .filter(|line| line.starts_with("receipt signal placement="))
        .collect::<Vec<_>>();
    assert_eq!(
        receipt_lines.len(),
        48,
        "expected three local show sinks to receipt sixteen signals each: {stdout}"
    );
    assert!(
        stdout.contains("receipts 48 first=(0, false) last=(15, true)"),
        "missing triple receipt summary: {stdout}"
    );
}

#[test]
#[cfg(feature = "sim-fixtures")]
fn observatory_fixture_report_is_explicitly_synthetic_and_does_not_run_work() {
    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("observatory-fixture-report")
        .output()
        .expect("failed to run conduit observatory report");

    assert!(output.status.success(), "process failed: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    assert!(
        stdout.contains("SIMULATION ONLY: synthetic observatory fixture"),
        "{stdout}"
    );
    assert!(stdout.contains("host observatory report"), "{stdout}");
    assert!(stdout.contains("hosts 3"), "{stdout}");
    assert!(
        stdout.contains("host id=std-host-triple boot=std-boot-triple"),
        "{stdout}"
    );
    assert!(
        stdout.contains("host id=browser-sim-triple boot=browser-sim-boot-triple"),
        "{stdout}"
    );
    assert!(
        stdout.contains("host id=pico-sim-triple boot=pico-sim-boot-triple"),
        "{stdout}"
    );
    assert!(stdout.contains("capabilities 6"), "{stdout}");
    assert!(stdout.contains("links 3"), "{stdout}");
    assert!(stdout.contains("plans 1"), "{stdout}");
    assert!(stdout.contains("placements 4"), "{stdout}");
    assert!(stdout.contains("connections 3"), "{stdout}");
    assert!(stdout.contains("provider=FixtureFrame"), "{stdout}");
    assert!(stdout.contains("provider=FixtureDatagram"), "{stdout}");
    assert!(
        stdout.contains("evidence id=") && stdout.contains("active_play=none presentation=none"),
        "{stdout}"
    );
    assert!(!stdout.contains("evidence id=evidence/"), "{stdout}");
    assert!(stdout.contains("retention bounded=true"), "{stdout}");
    assert!(
        !stdout.contains("receipt signal placement="),
        "observatory report must not activate work: {stdout}"
    );
}
