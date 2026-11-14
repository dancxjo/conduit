use super::*;
use conduit_std_host::{source_check::process::*, RunControl, RunControlRequestId};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
pub(super) fn worker(deadline: Duration, bytes: usize) -> SourceCheckProcessWorker {
    worker_at(
        std::path::Path::new(env!("CARGO_BIN_EXE_conduit")),
        deadline,
        bytes,
    )
}
fn worker_at(path: &std::path::Path, deadline: Duration, bytes: usize) -> SourceCheckProcessWorker {
    SourceCheckProcessWorker::new(
        path.to_path_buf(),
        SourceCheckProcessLimits {
            address_space_bytes: 512 * 1024 * 1024,
            cpu_seconds: 10,
        },
        deadline,
        bytes,
    )
    .unwrap()
}
fn children() -> String {
    std::fs::read_to_string("/proc/thread-self/children").unwrap()
}
#[test]
fn parent_owns_large_startup_and_retains_only_successful_canonical_output() {
    let (input, mut owner) = input(include_str!("../../../../plots/hello/main.conduit"));
    assert!(serde_json::to_vec(&input).unwrap().len() > 256);
    let expected = SourceCheckNativeSchema::prepare()
        .unwrap()
        .encode_reply(
            SourceCheckReply::Checked(&owner.project(&input.request).unwrap()),
            64 * 1024,
        )
        .unwrap();
    let before = children();
    let output = worker(Duration::from_secs(20), 16 * 1024)
        .prepare(&input, &RunControl::default())
        .unwrap();
    assert_eq!(output.canonical_bytes(), expected);
    assert_eq!(children(), before);
}
#[test]
fn stopped_and_oversized_startup_launch_nothing_and_deadline_reaps_the_real_child() {
    let (mut input, _) = input(include_str!("../../../../plots/hello/main.conduit"));
    let before = children();
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("before-source-child").unwrap())
        .unwrap();
    let mut absent = SourceCheckProcessWorker::new(
        PathBuf::from("/no-source-worker"),
        SourceCheckProcessLimits {
            address_space_bytes: 512 * 1024 * 1024,
            cpu_seconds: 10,
        },
        Duration::from_secs(20),
        16 * 1024,
    )
    .unwrap();
    assert!(matches!(
        absent.prepare(&input, &control),
        Err(SourceCheckProcessFailure::Cancelled)
    ));
    assert!(matches!(
        worker(Duration::from_secs(20), 1).prepare(&input, &RunControl::default()),
        Err(SourceCheckProcessFailure::InputBounds)
    ));
    assert!(matches!(
        worker(Duration::from_millis(1), 16 * 1024).prepare(&input, &RunControl::default()),
        Err(SourceCheckProcessFailure::Deadline)
    ));
    input.owner = "x".repeat(MAX_CHECK_LABEL_BYTES + 1);
    assert!(matches!(
        absent.prepare(&input, &RunControl::default()),
        Err(SourceCheckProcessFailure::InputBounds)
    ));
    assert_eq!(children(), before);
}
#[test]
fn stop_after_actual_child_appearance_discards_output_and_reaps() {
    let (input, _) = input(include_str!("../../../../plots/hello/main.conduit"));
    let before = children();
    assert!(before.trim().is_empty(), "test thread already has a child");
    let child_path = PathBuf::from("/proc")
        .join(std::fs::read_link("/proc/thread-self").unwrap())
        .join("children");
    let control = RunControl::default();
    let stop = control.clone();
    let monitor = std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end {
            if std::fs::read_to_string(&child_path).is_ok_and(|value| !value.trim().is_empty()) {
                stop.request_stop(RunControlRequestId::new("during-source-child").unwrap())
                    .unwrap();
                return true;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        false
    });
    let outcome = worker(Duration::from_secs(20), 16 * 1024).prepare(&input, &control);
    assert!(monitor.join().unwrap(), "no actual child observed");
    assert!(matches!(outcome, Err(SourceCheckProcessFailure::Cancelled)));
    assert_eq!(children(), before);
}

// Preserve the production artifact bound; debug-only sections are not part of
// the executable product being qualified here. One private copy per test run.
fn artifact() -> &'static std::path::Path {
    static ARTIFACT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ARTIFACT
        .get_or_init(|| {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/source-check-artifact")
                .join(std::process::id().to_string());
            std::fs::create_dir_all(&directory).unwrap();
            let output = directory.join("conduit");
            assert!(std::process::Command::new("strip")
                .arg("--strip-debug")
                .arg("-o")
                .arg(&output)
                .arg(env!("CARGO_BIN_EXE_conduit"))
                .status()
                .unwrap()
                .success());
            output
        })
        .as_path()
}
fn executable_digest(path: &std::path::Path) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path).unwrap();
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let n = file.read(&mut buffer).unwrap();
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    hash.finalize().into()
}
#[test]
fn selected_artifact_identity_is_checked_before_source_delivery() {
    let (input, _) = input(include_str!("../../../../plots/hello/main.conduit"));
    let before = children();
    let path = artifact();
    let digest = executable_digest(path);
    let mut exact = worker_at(path, Duration::from_secs(20), 16 * 1024)
        .with_executable_identity(digest, 512 * 1024 * 1024)
        .unwrap();
    assert!(exact.prepare(&input, &RunControl::default()).is_ok());
    let mut wrong = worker_at(path, Duration::from_secs(20), 16 * 1024)
        .with_executable_identity([0; 32], 512 * 1024 * 1024)
        .unwrap();
    assert!(matches!(
        wrong.prepare(&input, &RunControl::default()),
        Err(SourceCheckProcessFailure::ExecutableIdentity)
    ));
    let mut too_large = worker_at(path, Duration::from_secs(20), 16 * 1024)
        .with_executable_identity(digest, 1)
        .unwrap();
    assert!(matches!(
        too_large.prepare(&input, &RunControl::default()),
        Err(SourceCheckProcessFailure::ExecutableBounds)
    ));
    assert_eq!(children(), before);
}
pub(super) fn configuration_source() -> String {
    let path = artifact();
    let digest = executable_digest(path)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!(
        r#"host compiler {{
      schema = 1
      target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
      base = {{kind: "language/source-compiler", implementation: "std/bounded-source-check@1", initialization: {{executable: {}, executable_sha256: "{digest}", maximum_executable_bytes: 536870912, address_space_bytes: 536870912, cpu_seconds: 10, deadline_milliseconds: 20000, maximum_startup_bytes: 16384, preparation_bytes: 570425344, retained_bytes: 1048576, request_bytes: 16384, reply_bytes: 65536}}}}
      limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 603979776, queue_items: 32, buffered_bytes: 1048576, active_instances: 16, operation_slots: 16, timer_slots: 16, line_sessions: 16, evidence_items: 16}}
    }}"#,
        serde_json::to_string(path.to_str().unwrap()).unwrap()
    )
}
#[test]
#[cfg(target_arch = "x86_64")]
fn checked_host_constructs_the_actual_digest_bound_worker() {
    let source = configuration_source();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let mut worker = conduit::source_check_worker::from_configuration(&checked).unwrap();
    let (input, _) = input(include_str!("../../../../plots/hello/main.conduit"));
    assert!(worker.prepare(&input, &RunControl::default()).is_ok());
}

#[test]
fn reply_capacity_is_an_actual_parent_bound() {
    let (input, _) = input(include_str!("../../../../plots/hello/main.conduit"));
    let before = children();
    let mut selected = worker(Duration::from_secs(20), 16 * 1024)
        .with_reply_bound(1)
        .unwrap();
    assert!(matches!(
        selected.prepare(&input, &RunControl::default()),
        Err(SourceCheckProcessFailure::OutputBounds)
    ));
    assert_eq!(children(), before);
}
