use super::*;
use crate::commands::pete_std_test_support::Pty;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-create-speaker-{name}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}

fn args(serial_path: PathBuf, evidence_out: PathBuf) -> StdSpeakerArgs {
    StdSpeakerArgs {
        serial_path,
        base_id: "std/create-uart/test".into(),
        host_id: "std-host/test".into(),
        boot_id: "std-boot/test".into(),
        robot_id: "robot/create1/test".into(),
        attest_robot_identity: true,
        read_timeout_ms: 500,
        evidence_out,
    }
}

fn read_exact(master: &mut std::fs::File, expected: &[u8]) {
    let mut observed = vec![0_u8; expected.len()];
    master.read_exact(&mut observed).unwrap();
    assert_eq!(observed, expected);
}

#[test]
fn exact_std_path_runs_portable_plan_and_only_speaker_bytes() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let (response_consumed_tx, response_consumed_rx) = std::sync::mpsc::channel();
    let responder = std::thread::spawn(move || {
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[132]);
        read_exact(&mut pty.master, &[142, 35]);
        pty.master.write_all(&[3]).unwrap();
        read_exact(&mut pty.master, &[140, 2, 4, 60, 32, 64, 32, 0, 12, 67, 40]);
        read_exact(&mut pty.master, &[141, 2]);
        read_exact(&mut pty.master, &[142, 36]);
        pty.master.write_all(&[2]).unwrap();
        read_exact(&mut pty.master, &[142, 37]);
        pty.master.write_all(&[1]).unwrap();
        read_exact(&mut pty.master, &[142, 37]);
        pty.master.write_all(&[0]).unwrap();
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[131]);
        read_exact(&mut pty.master, &[142, 35]);
        pty.master.write_all(&[2]).unwrap();
        // Keep both PTY ends alive until the command has consumed the final
        // byte. Closing the master immediately after writing can surface HUP
        // before the slave drains the response on Linux.
        response_consumed_rx.recv().unwrap();
    });
    let evidence = execute(&args(path, temp_path("unused"))).unwrap();
    response_consumed_tx.send(()).unwrap();
    responder.join().unwrap();
    assert!(matches!(evidence.outcome, Outcome::Completed));
    assert_eq!(evidence.post_bound_song_playing, Some(false));
    assert_eq!(evidence.final_oi_mode, "safe");
    assert!(evidence.safe_cleanup_completed);
    assert!(!evidence.motion_authority_granted);
    assert!(evidence.kernel_decisions > 0);
    assert!(evidence.kernel_signs > 0);
    assert_eq!(evidence.audibility, "pending_operator_confirmation");
    for forbidden in ["create", "uart", "serial", "speaker", "pete"] {
        assert!(!evidence
            .portable_form
            .to_ascii_lowercase()
            .contains(forbidden));
    }
}

#[test]
fn failed_safe_cleanup_is_never_reported_as_completed() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let responder = std::thread::spawn(move || {
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[132]);
        read_exact(&mut pty.master, &[142, 35]);
        pty.master.write_all(&[3]).unwrap();
        read_exact(&mut pty.master, &[140, 2, 4, 60, 32, 64, 32, 0, 12, 67, 40]);
        read_exact(&mut pty.master, &[141, 2]);
        read_exact(&mut pty.master, &[142, 36]);
        pty.master.write_all(&[2]).unwrap();
        read_exact(&mut pty.master, &[142, 37]);
        pty.master.write_all(&[1]).unwrap();
        read_exact(&mut pty.master, &[142, 37]);
        pty.master.write_all(&[0]).unwrap();
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[131]);
        read_exact(&mut pty.master, &[142, 35]);
        // The device claims it remained FULL after the mandatory SAFE request.
        pty.master.write_all(&[3]).unwrap();
    });
    let evidence = execute(&args(path, temp_path("unused-cleanup-failure"))).unwrap();
    responder.join().unwrap();
    assert!(matches!(
        evidence.outcome,
        Outcome::Failed {
            stage: "restore_safe",
            ..
        }
    ));
    assert_eq!(evidence.final_oi_mode, "unknown");
    assert!(!evidence.safe_cleanup_completed);
}

#[test]
fn failed_full_observation_still_restores_and_observes_safe() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let (response_consumed_tx, response_consumed_rx) = std::sync::mpsc::channel();
    let responder = std::thread::spawn(move || {
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[132]);
        read_exact(&mut pty.master, &[142, 35]);
        // This is a complete one-byte mode packet, but not a legal OI mode.
        pty.master.write_all(&[0xff]).unwrap();
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[131]);
        read_exact(&mut pty.master, &[142, 35]);
        pty.master.write_all(&[2]).unwrap();
        response_consumed_rx.recv().unwrap();
    });
    let error = match execute(&args(path, temp_path("unused-full-failure"))) {
        Ok(_) => panic!("malformed Full observation must fail"),
        Err(error) => error.to_string(),
    };
    response_consumed_tx.send(()).unwrap();
    responder.join().unwrap();
    assert!(
        error.contains("speaker mode acquisition failed:"),
        "unexpected error: {error}"
    );
    assert!(
        error.contains("Safe cleanup completed"),
        "unexpected error: {error}"
    );
}

#[test]
fn identity_attestation_is_required_before_device_or_evidence_access() {
    let evidence = temp_path("unattested-evidence");
    let mut input = args(temp_path("absent-device"), evidence.clone());
    input.attest_robot_identity = false;
    assert!(run(
        input,
        &GlobalOpts {
            dry_run: false,
            quiet: true,
            json: false,
            locked: false,
        }
    )
    .is_err());
    assert!(!evidence.exists());
}

#[test]
fn evidence_publication_is_atomic_and_never_overwrites() {
    let path = temp_path("atomic-evidence");
    write_new_atomic(&path, b"complete").unwrap();
    assert!(write_new_atomic(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"complete");
    std::fs::remove_file(path).unwrap();
}
