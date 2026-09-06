use super::*;
use crate::commands::pete_std_test_support::Pty;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

fn args(serial_path: PathBuf, evidence_out: PathBuf) -> StdObserveArgs {
    StdObserveArgs {
        serial_path,
        base_id: "std/create-uart/test".into(),
        host_id: "std-host/test".into(),
        boot_id: "std-boot/test".into(),
        read_timeout_ms: 500,
        evidence_out,
    }
}

fn frame() -> Vec<u8> {
    let mut group = [0_u8; 26];
    group[0] = 0b0000_0010;
    group[11] = 1;
    group[12..14].copy_from_slice(&12_i16.to_be_bytes());
    group[14..16].copy_from_slice(&(-3_i16).to_be_bytes());
    group[16] = 3;
    group[17..19].copy_from_slice(&14_000_u16.to_be_bytes());
    group[19..21].copy_from_slice(&100_i16.to_be_bytes());
    group[21] = 24;
    group[22..24].copy_from_slice(&1_000_u16.to_be_bytes());
    group[24..26].copy_from_slice(&2_000_u16.to_be_bytes());
    let mut frame = vec![19, 29, 0];
    frame.extend_from_slice(&group);
    frame.extend_from_slice(&[34, 2]);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    frame
}

fn battery_estimate_frame(charge_mah: u16, capacity_mah: u16) -> Vec<u8> {
    let mut response = frame();
    response[25..27].copy_from_slice(&charge_mah.to_be_bytes());
    response[27..29].copy_from_slice(&capacity_mah.to_be_bytes());
    let checksum = response.len() - 1;
    response[checksum] = 0;
    let sum = response
        .iter()
        .fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    response[checksum] = 0_u8.wrapping_sub(sum);
    response
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-pete-{name}-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn pseudo_terminal_resynchronizes_exact_non_actuating_session_without_identity_claim() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let response = frame();
    let responder = std::thread::spawn(move || {
        let mut start = [0_u8; 6];
        pty.master.read_exact(&mut start).unwrap();
        assert_eq!(start, [128, 131, 148, 2, 0, 34]);
        pty.master.write_all(&[0xaa, 0xbb, 0xcc]).unwrap();
        pty.master.write_all(&response).unwrap();
        let mut pause = [0_u8; 2];
        pty.master.read_exact(&mut pause).unwrap();
        assert_eq!(pause, [150, 0]);
    });
    let evidence = execute(&args(path, temp_path("unused"))).unwrap();
    responder.join().unwrap();
    assert!(!evidence.intended_robot_identity_verified);
    assert_eq!(
        evidence.schema,
        "conduit.pete/std-create-observation-evidence@4"
    );
    assert_eq!(evidence.stream_maximum_discarded_bytes, 64);
    assert_eq!(
        evidence.proof_class,
        "live_host_boundary_unverified_robot_identity"
    );
    match evidence.outcome {
        Outcome::Observed { observation } => {
            assert_eq!(observation.stream_discarded_bytes, 3);
            assert_eq!(observation.contact_body_sectors, 2);
            assert_eq!(observation.charging_sources, 2);
            assert_eq!(observation.battery_charge_permille, Some(500));
            assert_eq!(observation.distance_delta_mm, 12);
            assert_eq!(observation.start_local_odometry.frame_generation, 1);
            assert_eq!(observation.start_local_odometry.sample_generation, 1);
            assert_eq!(observation.start_local_odometry.forward_mm, 12);
            assert_eq!(observation.start_local_odometry.lateral_mm, 0);
            assert_eq!(observation.start_local_odometry.yaw_microradians, -52_360);
        }
        Outcome::Failed { .. } => panic!("pseudo-terminal observation failed"),
    }
}

#[test]
fn silent_character_device_is_device_no_response_and_still_pauses() {
    let mut pty = Pty::open();
    let path = pty.slave_path.clone();
    let responder = std::thread::spawn(move || {
        let mut start = [0_u8; 6];
        pty.master.read_exact(&mut start).unwrap();
        let mut pause = [0_u8; 2];
        pty.master.read_exact(&mut pause).unwrap();
        assert_eq!(pause, [150, 0]);
    });
    let mut input = args(path, temp_path("unused"));
    input.read_timeout_ms = 20;
    let evidence = execute(&input).unwrap();
    responder.join().unwrap();
    assert!(matches!(
        evidence.outcome,
        Outcome::Failed {
            stage: "session_read",
            code: "device_no_response",
            cleanup_code: None
        }
    ));
}

#[test]
fn create_1_battery_estimate_quirks_are_normalized_and_still_pause() {
    for (charge_mah, capacity_mah, normalized_charge, normalized_capacity, disposition) in [
        (
            2_001,
            2_000,
            2_000,
            2_000,
            "charge_saturated_to_estimated_capacity",
        ),
        (1_000, 0, 0, 0, "estimated_capacity_unavailable"),
    ] {
        let mut pty = Pty::open();
        let path = pty.slave_path.clone();
        let response = battery_estimate_frame(charge_mah, capacity_mah);
        let responder = std::thread::spawn(move || {
            let mut start = [0_u8; 6];
            pty.master.read_exact(&mut start).unwrap();
            assert_eq!(start, [128, 131, 148, 2, 0, 34]);
            pty.master.write_all(&response).unwrap();
            let mut pause = [0_u8; 2];
            pty.master.read_exact(&mut pause).unwrap();
            assert_eq!(pause, [150, 0]);
        });

        let evidence = execute(&args(path, temp_path("unused"))).unwrap();
        responder.join().unwrap();
        match evidence.outcome {
            Outcome::Observed { observation } => {
                assert_eq!(observation.reported_charge_mah, charge_mah);
                assert_eq!(observation.reported_capacity_mah, capacity_mah);
                assert_eq!(observation.portable_charge_mah, normalized_charge);
                assert_eq!(observation.portable_capacity_mah, normalized_capacity);
                assert_eq!(observation.battery_normalization, disposition);
            }
            Outcome::Failed { .. } => panic!("canonical Create 1 estimate was refused"),
        }
    }
}

#[test]
fn dry_run_touches_neither_serial_path_nor_evidence_destination() {
    let evidence = temp_path("dry-evidence");
    let input = args(temp_path("absent-serial"), evidence.clone());
    run_std_observe(
        input,
        &GlobalOpts {
            dry_run: true,
            quiet: true,
            json: false,
            locked: false,
        },
    )
    .unwrap();
    assert!(!evidence.exists());
}

#[test]
fn unrelated_non_character_device_retains_failure_not_physical_success() {
    let serial = temp_path("regular-file");
    let evidence = temp_path("regular-file-evidence");
    std::fs::write(&serial, b"not a serial device").unwrap();
    let result = run_std_observe(
        args(serial.clone(), evidence.clone()),
        &GlobalOpts {
            dry_run: false,
            quiet: true,
            json: false,
            locked: false,
        },
    );
    assert!(result.is_err());
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&evidence).unwrap()).unwrap();
    assert_eq!(report["intended_robot_identity_verified"], false);
    assert_eq!(report["outcome"]["status"], "failed");
    assert_eq!(report["outcome"]["stage"], "base_open");
    assert_eq!(report["outcome"]["code"], "not_character_device");
    std::fs::remove_file(serial).unwrap();
    std::fs::remove_file(evidence).unwrap();
}

#[test]
fn evidence_publication_is_atomic_and_never_overwrites_history() {
    let path = temp_path("atomic-evidence");
    write_new_atomic(&path, b"complete").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"complete");
    assert!(write_new_atomic(&path, b"replacement").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"complete");
    std::fs::remove_file(path).unwrap();
}
