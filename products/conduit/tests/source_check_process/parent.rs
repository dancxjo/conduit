use super::*;
use conduit_std_host::{source_check::process::*, RunControl, RunControlRequestId};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn worker(deadline: Duration, bytes: usize) -> SourceCheckProcessWorker {
    SourceCheckProcessWorker::new(
        PathBuf::from(env!("CARGO_BIN_EXE_conduit")),
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
