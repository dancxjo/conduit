use super::*;
use crate::commands::pete_std_test_support::Pty;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-create-indicator-{name}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}

fn args(serial_path: PathBuf, evidence_out: PathBuf) -> StdIndicatorArgs {
    StdIndicatorArgs {
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
fn exact_std_path_runs_canonical_signal_and_finishes_indicator_off() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let (consumed_tx, consumed_rx) = std::sync::mpsc::channel();
    let responder = std::thread::spawn(move || {
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[131]);
        read_exact(&mut pty.master, &[142, 35]);
        pty.master.write_all(&[2]).unwrap();
        for sequence in 0..16 {
            let expected = if sequence % 2 == 0 {
                [139, 0, 0, 0]
            } else {
                [139, 0, 0, 255]
            };
            read_exact(&mut pty.master, &expected);
        }
        read_exact(&mut pty.master, &[139, 0, 0, 0]);
        consumed_rx.recv().unwrap();
    });
    let evidence = execute(&args(path, temp_path("unused"))).unwrap();
    consumed_tx.send(()).unwrap();
    responder.join().unwrap();
    assert!(matches!(evidence.outcome, Outcome::Completed));
    assert_eq!(evidence.receipts.len(), 16);
    assert_eq!(evidence.receipts[0].sequence, 0);
    assert!(!evidence.receipts[0].level);
    assert_eq!(evidence.receipts[15].sequence, 15);
    assert!(evidence.receipts[15].level);
    assert_eq!(evidence.indicator_commands, 16);
    assert!(evidence.final_off.attempted);
    assert!(evidence.final_off.committed);
    assert!(!evidence.motion_authority_granted);
    assert!(evidence.kernel_decisions > 0);
    assert!(evidence.kernel_signs > 0);
    assert_eq!(
        evidence.operator_visibility,
        "pending_operator_confirmation"
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
fn provider_loss_during_manifestation_retains_failed_cleanup() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let responder = std::thread::spawn(move || {
        read_exact(&mut pty.master, &[128]);
        read_exact(&mut pty.master, &[131]);
        read_exact(&mut pty.master, &[142, 35]);
        pty.master.write_all(&[2]).unwrap();
        read_exact(&mut pty.master, &[139, 0, 0, 0]);
        // Dropping both PTY ends makes the next manifestation and the
        // mandatory terminal-off cleanup fail at the provider seam.
    });
    let evidence = execute(&args(path, temp_path("unused"))).unwrap();
    responder.join().unwrap();
    assert!(matches!(
        evidence.outcome,
        Outcome::Failed {
            stage: "kernel_signal_play",
            ..
        }
    ));
    assert!(evidence.final_off.attempted);
    assert!(!evidence.final_off.committed);
    assert!(!evidence.motion_authority_granted);
}

#[test]
fn evidence_publication_is_atomic_and_never_overwrites() {
    let path = temp_path("atomic-evidence");
    write_new_atomic(&path, b"complete").unwrap();
    assert!(write_new_atomic(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"complete");
    std::fs::remove_file(path).unwrap();
}
