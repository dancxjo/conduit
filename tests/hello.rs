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
fn triple_signal_form_runs_against_local_std_fixture() {
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
fn observatory_report_is_operator_openable_without_running_work() {
    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("observatory-report")
        .output()
        .expect("failed to run conduit observatory report");

    assert!(output.status.success(), "process failed: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    assert!(stdout.contains("host observatory report"), "{stdout}");
    assert!(stdout.contains("hosts 3"), "{stdout}");
    assert!(
        stdout.contains("host id=std-host-triple boot=std-boot-triple"),
        "{stdout}"
    );
    assert!(
        stdout.contains("host id=browser-host-triple boot=browser-boot-triple"),
        "{stdout}"
    );
    assert!(
        stdout.contains("host id=pico-host-triple boot=pico-boot-triple"),
        "{stdout}"
    );
    assert!(stdout.contains("capabilities 6"), "{stdout}");
    assert!(stdout.contains("links 3"), "{stdout}");
    assert!(stdout.contains("plans 1"), "{stdout}");
    assert!(stdout.contains("placements 4"), "{stdout}");
    assert!(stdout.contains("connections 3"), "{stdout}");
    assert!(stdout.contains("provider=WebSocket"), "{stdout}");
    assert!(stdout.contains("provider=Udp"), "{stdout}");
    assert!(stdout.contains("evidence id=evidence/"), "{stdout}");
    assert!(stdout.contains("retention bounded=true"), "{stdout}");
    assert!(
        !stdout.contains("receipt signal placement="),
        "observatory report must not activate work: {stdout}"
    );
}

#[test]
fn copy_file_command_runs_task_before_revealing_form_and_plan() {
    let temp_dir = std::env::temp_dir().join(format!("conduit-copy-cli-{}", std::process::id()));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).expect("old temp removed");
    }
    std::fs::create_dir_all(&temp_dir).expect("temp dir created");
    let source = temp_dir.join("source.txt");
    let destination = temp_dir.join("destination.txt");
    std::fs::write(&source, "copy through conduit\n").expect("source written");

    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "copy-file",
            "--source",
            source.to_str().expect("source path utf-8"),
            "--destination",
            destination.to_str().expect("destination path utf-8"),
            "--inspect",
        ])
        .output()
        .expect("failed to run conduit copy-file");

    assert!(output.status.success(), "process failed: {output:?}");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("destination copied"),
        "copy through conduit\n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    assert!(stdout.contains("preflight will-create"), "{stdout}");
    assert!(stdout.contains("primary-action Run/Stop"), "{stdout}");
    assert!(
        stdout.contains("result success-created bytes=21"),
        "{stdout}"
    );
    assert!(stdout.contains("receipt request="), "{stdout}");
    assert!(stdout.contains("source-binding="), "{stdout}");
    assert!(stdout.contains("destination-binding="), "{stdout}");
    assert!(stdout.contains("inspect form-source begin"), "{stdout}");
    assert!(stdout.contains("copy: task/copy-file"), "{stdout}");
    assert!(stdout.contains("inspect plan "), "{stdout}");
    let inspected_form = stdout
        .split("inspect form-source begin")
        .nth(1)
        .and_then(|tail| tail.split("inspect form-source end").next())
        .expect("inspect form block present");
    assert!(
        !inspected_form.contains(source.to_str().unwrap()),
        "{stdout}"
    );
    assert!(
        !inspected_form.contains(destination.to_str().unwrap()),
        "{stdout}"
    );
}
