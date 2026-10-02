use std::path::PathBuf;
use std::process::Command;

fn unique_report_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-{name}-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

fn unique_artifact_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ))
}

#[test]
fn canonical_hello_runs_locally() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let plot_path = workspace_root.join("plots/hello/main.conduit");

    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(["run", plot_path.to_str().expect("plot path must be utf-8")])
        .output()
        .expect("failed to run conduit binary");

    assert!(output.status.success(), "process failed: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("stdout must be utf-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 9, "unexpected output: {stdout}");
    assert!(
        lines
            .first()
            .is_some_and(|line| line.starts_with("host std-host-1 boot boot-")
                && line.ends_with(" profile rust-std protocol 1")),
        "missing fresh host/boot report: {stdout}"
    );
    assert!(
        lines.iter().any(
            |line| line.contains("kind=text/upper host=std-host-1 boot=boot-")
                && line.contains(" implementation=std/kernel-text-upper@1 ")
        ),
        "missing canonical upper placement: {stdout}"
    );
    assert!(
        lines.iter().any(
            |line| line.contains("kind=presentation/text host=std-host-1 boot=boot-")
                && line.contains(" implementation=std/kernel-presentation-text@1 ")
        ),
        "missing canonical presentation placement: {stdout}"
    );
    assert!(lines.iter().any(|line| line == &"HELLO, WORLD."));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("plan ") && line.ends_with(" complete")),
        "missing plan completion line: {stdout}"
    );
}

#[test]
fn actual_std_run_writes_a_read_only_observatory_report() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let plot_path = workspace_root.join("plots/hello/main.conduit");
    let report_path = unique_report_path("actual-observatory");
    let artifact_directory = unique_artifact_directory("actual-execution-artifacts");
    let _ = std::fs::remove_file(&report_path);
    let _ = std::fs::remove_dir_all(&artifact_directory);

    let run = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "run",
            plot_path.to_str().expect("plot path must be utf-8"),
            "--report",
            report_path.to_str().expect("report path must be utf-8"),
            "--artifacts",
            artifact_directory
                .to_str()
                .expect("artifact directory must be utf-8"),
        ])
        .output()
        .expect("failed to run conduit binary");
    assert!(run.status.success(), "actual run failed: {run:?}");
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("HELLO, WORLD."),
        "the producing command must be the actual std execution path"
    );

    let mut artifact: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&report_path).expect("runtime report artifact must exist"),
    )
    .expect("runtime report artifact must be json");
    assert_eq!(artifact["schema"], "conduit.observatory.snapshot/v2");
    assert_eq!(artifact["hosts"].as_array().unwrap().len(), 1);
    assert_eq!(artifact["plans"].as_array().unwrap().len(), 1);
    let observations = artifact["observations"].as_array().unwrap();
    assert!(!observations.is_empty());
    let sign_ids = observations
        .iter()
        .map(|observation| {
            observation["sign_id"]
                .as_str()
                .expect("runtime observation has an sign identity")
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(sign_ids.len(), observations.len());
    assert!(sign_ids.iter().all(|identity| identity.len() == 64));
    assert!(observations
        .iter()
        .any(|observation| observation["active_play_id"].as_str().is_some()));

    let retained = std::fs::read_dir(&artifact_directory)
        .expect("standalone execution artifact directory must exist")
        .map(|entry| entry.unwrap().path())
        .collect::<Vec<_>>();
    let artifact_with_prefix = |prefix: &str| {
        retained
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|name| name.starts_with(prefix))
            })
            .expect("requested standalone artifact must exist")
    };
    let plan_artifact = artifact_with_prefix("plan-");
    let play_artifact = artifact_with_prefix("play-");
    let sign_artifact = artifact_with_prefix("sign-");
    assert_eq!(
        retained
            .iter()
            .filter(|path| path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("plan-"))
            .count(),
        1
    );
    assert_eq!(
        retained
            .iter()
            .filter(|path| path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("play-"))
            .count(),
        1
    );
    assert_eq!(
        retained
            .iter()
            .filter(|path| path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("sign-"))
            .count(),
        observations.len()
    );
    for (path, expected) in [
        (plan_artifact, "Plan "),
        (play_artifact, "Play "),
        (sign_artifact, "Sign "),
    ] {
        let inspected = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(path)
            .output()
            .expect("standalone execution artifact inspection runs");
        assert!(
            inspected.status.success(),
            "inspection failed: {inspected:?}"
        );
        assert!(String::from_utf8(inspected.stdout)
            .unwrap()
            .starts_with(expected));
    }

    let inspect = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "inspect",
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
    assert!(stdout.contains("sign slots"), "{stdout}");
    assert!(
        !stdout.contains("receipt signal placement=") && !stdout.contains("signal 0 off"),
        "read-only inspection must not trigger work: {stdout}"
    );

    artifact["retention"]["dropped_items"] = serde_json::Value::from(7);
    std::fs::write(
        &report_path,
        serde_json::to_vec_pretty(&artifact).expect("gap report remains json"),
    )
    .expect("gap report can be written");
    let gap_report = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args([
            "inspect",
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
            "inspect",
            report_path.to_str().expect("report path must be utf-8"),
        ])
        .output()
        .expect("failed to inspect tampered report");
    let _ = std::fs::remove_file(&report_path);
    let _ = std::fs::remove_dir_all(&artifact_directory);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("unreported host/boot"),
        "tampered identity rejection was not explicit: {rejected:?}"
    );
}

#[test]
fn product_run_refuses_precanonical_fixture_source() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist");
    let canonical = workspace_root.join("plots/hello/main.conduit");
    let plot_path = unique_report_path("noncanonical-source").with_extension("plot");
    std::fs::copy(canonical, &plot_path).expect("temporary noncanonical source copies");
    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .args(["run", plot_path.to_str().expect("plot path must be utf-8")])
        .output()
        .expect("failed to run conduit binary");
    let _ = std::fs::remove_file(&plot_path);

    assert!(!output.status.success(), "fixture source unexpectedly ran");
    let stderr = String::from_utf8(output.stderr).expect("stderr must be utf-8");
    assert!(
        stderr.contains("canonical Plot source must use the .conduit suffix"),
        "unexpected refusal: {stderr}"
    );
}
