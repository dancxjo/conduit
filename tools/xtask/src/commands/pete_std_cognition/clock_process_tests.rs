//! Actual same-kernel, distinct-process identity; no namespace mutation or model/device access.
use super::*;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const PREFIX: &str = "CONDUIT_PETE_CLOCK_CHILD=";

#[test]
fn child_process_clock_identity() {
    let mut clock = PreparedClock::prepare().expect("Linux clock domain must be observable");
    let ticks = clock.millis().unwrap();
    println!(
        "\n{PREFIX}{}",
        serde_json::json!({"basis":clock.basis(),"ticks":ticks,"pid":std::process::id()})
    );
}

#[test]
fn independent_processes_share_the_same_prepared_kernel_clock_domain() {
    let mut parent = PreparedClock::prepare().unwrap();
    let before = parent.millis().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "commands::pete_std_cognition::clock::process_tests::child_process_clock_identity",
            "--nocapture",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "clock child exceeded 10s: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    let after = parent.millis().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let receipts: Vec<_> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect();
    assert_eq!(receipts.len(), 1, "{stdout}");
    let receipt: serde_json::Value = serde_json::from_str(receipts[0]).unwrap();
    println!(
        "CONDUIT_PETE_CLOCK_PROCESS_EVIDENCE={}",
        serde_json::json!({
            "proof_class":"actual_distinct_process_same_kernel_clock", "parent_pid":std::process::id(),
            "child":receipt, "parent_basis":parent.basis(), "before":before,"after":after,
        })
    );
    assert_ne!(receipt["pid"], std::process::id());
    assert_eq!(receipt["basis"], parent.basis());
    let ticks = receipt["ticks"].as_u64().unwrap();
    assert!(before <= ticks && ticks <= after);
}
