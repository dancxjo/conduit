use std::path::PathBuf;
use std::process::Command;

fn unique_report_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-{name}-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

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
fn actual_std_run_writes_a_read_only_observatory_report() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let form_path = workspace_root.join("examples/signal-demo.form");
    let placements_path = workspace_root.join("examples/std-local.placements");
    let report_path = unique_report_path("actual-observatory");
    let _ = std::fs::remove_file(&report_path);

    let run = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            form_path.to_str().expect("form path must be utf-8"),
            "--placements",
            placements_path
                .to_str()
                .expect("placements path must be utf-8"),
            "--report",
            report_path.to_str().expect("report path must be utf-8"),
        ])
        .output()
        .expect("failed to run conduit binary");
    assert!(run.status.success(), "actual run failed: {run:?}");
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("receipt signal placement="),
        "the producing command must be the actual std execution path"
    );

    let mut artifact: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&report_path).expect("runtime report artifact must exist"),
    )
    .expect("runtime report artifact must be json");
    assert_eq!(artifact["schema"], "conduit.observatory.snapshot/v1");
    assert_eq!(artifact["hosts"].as_array().unwrap().len(), 1);
    assert_eq!(artifact["plans"].as_array().unwrap().len(), 1);
    let observations = artifact["observations"].as_array().unwrap();
    assert!(!observations.is_empty());
    let evidence_ids = observations
        .iter()
        .map(|observation| {
            observation["evidence_id"]
                .as_str()
                .expect("runtime observation has an evidence identity")
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(evidence_ids.len(), observations.len());
    assert!(evidence_ids.iter().all(|identity| identity.len() == 64));
    assert!(observations.iter().any(|observation| {
        observation["active_play_id"].as_str().is_some()
            && observation["presentation_id"].as_str().is_some()
    }));

    let inspect = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "observatory-report",
            report_path.to_str().expect("report path must be utf-8"),
        ])
        .output()
        .expect("failed to inspect runtime report");
    assert!(inspect.status.success(), "inspection failed: {inspect:?}");
    let stdout = String::from_utf8(inspect.stdout).expect("report must be utf-8");
    assert!(stdout.contains("host observatory report"), "{stdout}");
    assert!(stdout.contains("hosts 1"), "{stdout}");
    assert!(stdout.contains("plans 1"), "{stdout}");
    assert!(stdout.contains("fragments 1"), "{stdout}");
    assert!(stdout.contains("plays 1"), "{stdout}");
    assert!(
        stdout.contains("lifecycle=Completed terminal=Some(Completed)"),
        "{stdout}"
    );
    assert!(stdout.contains("play-placement play="), "{stdout}");
    assert!(stdout.contains("play-connection play="), "{stdout}");
    assert!(
        stdout.contains("placement=")
            && stdout.contains("lifecycle=Completed terminal=Some(Completed)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("connection=")
            && stdout.contains("lifecycle=Completed terminal=None pressure=unknown"),
        "{stdout}"
    );
    assert!(stdout.contains("pressure=unknown"), "{stdout}");
    assert!(
        stdout.contains("active_play=") && stdout.contains("presentation="),
        "{stdout}"
    );
    assert!(stdout.contains("retained="), "{stdout}");
    assert!(stdout.contains("evidence slots"), "{stdout}");
    assert!(
        !stdout.contains("receipt signal placement=") && !stdout.contains("signal 0 off"),
        "read-only inspection must not activate work: {stdout}"
    );

    artifact["retention"]["dropped_items"] = serde_json::Value::from(7);
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&artifact).expect("gap report remains json"),
    )
    .expect("gap report can be written");
    let gap_report = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "observatory-report",
            report_path.to_str().expect("report path must be utf-8"),
        ])
        .output()
        .expect("failed to inspect runtime report with retention loss");
    assert!(
        gap_report.status.success(),
        "gap report failed: {gap_report:?}"
    );
    let gap_stdout = String::from_utf8(gap_report.stdout).expect("gap report must be utf-8");
    assert!(gap_stdout.contains("visible_gaps=7"), "{gap_stdout}");
    assert!(gap_stdout.contains("snapshot dropped 7"), "{gap_stdout}");

    artifact["observations"][0]["boot_id"] = serde_json::Value::String("stale-boot".into());
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&artifact).expect("tampered report remains json"),
    )
    .expect("tampered report can be written");
    let rejected = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "observatory-report",
            report_path.to_str().expect("report path must be utf-8"),
        ])
        .output()
        .expect("failed to inspect tampered report");
    let _ = std::fs::remove_file(&report_path);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("unreported host/boot"),
        "tampered identity rejection was not explicit: {rejected:?}"
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
