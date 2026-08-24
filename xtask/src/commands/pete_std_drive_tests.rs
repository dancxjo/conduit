use super::*;
use crate::commands::pete_std_test_support::Pty;
use std::io::{Read, Write};
use std::thread;

fn args() -> StdDriveArgs {
    StdDriveArgs {
        serial_path: "/dev/ttyUSB0".into(),
        base_id: "std/create-uart/0".into(),
        host_id: "std-host/0".into(),
        boot_id: "std-boot/0".into(),
        robot_id: "robot/create1/0".into(),
        attest_robot_identity: true,
        motion_environment: MotionEnvironment::WheelsOffFloor,
        confirm_wheels_off_floor: true,
        reduced_safety_floor_ack: None,
        read_timeout_ms: 1_000,
        evidence_out: "target/pete-drive.json".into(),
    }
}

#[test]
fn default_proof_refuses_without_exact_wheels_off_floor_attestation() {
    let mut value = args();
    value.confirm_wheels_off_floor = false;
    assert!(validate(&value)
        .unwrap_err()
        .to_string()
        .contains("--confirm-wheels-off-floor"));
}

#[test]
fn floor_ack_token_binds_every_exact_attachment_and_plan_identity() {
    let value = args();
    let token = floor_ack_token(&value, "plan/one");
    for mutate in [
        ("host", "std-host/other"),
        ("boot", "std-boot/other"),
        ("base", "std/create-uart/other"),
        ("robot", "robot/create1/other"),
    ] {
        let mut changed = value.clone();
        match mutate.0 {
            "host" => changed.host_id = mutate.1.into(),
            "boot" => changed.boot_id = mutate.1.into(),
            "base" => changed.base_id = mutate.1.into(),
            "robot" => changed.robot_id = mutate.1.into(),
            _ => unreachable!(),
        }
        assert_ne!(floor_ack_token(&changed, "plan/one"), token);
    }
    assert_ne!(floor_ack_token(&value, "plan/two"), token);
}

#[test]
fn exact_pty_path_runs_kernel_motion_then_ttl_zero_and_post_observation() {
    let pty = Pty::open();
    let mut value = args();
    value.serial_path = pty.slave_path.clone();
    let _slave_guard = pty.slave_guard;
    let peer = thread::spawn(move || success_script(pty.master));
    let evidence = execute(&value).unwrap();
    peer.join().unwrap();
    assert!(matches!(evidence.outcome, Outcome::Completed));
    assert!(matches!(
        evidence.dispatch.as_ref().map(|report| report.terminal.as_str()),
        Some(value) if value.contains("MotionAdmitted")
    ));
    assert!(matches!(
        evidence.terminal.as_ref().map(|report| report.terminal.as_str()),
        Some(value) if value.contains("DeadlineExpired")
    ));
    assert_eq!(
        evidence
            .post_observation
            .expect("post observation")
            .distance_delta_mm,
        12
    );
}

#[test]
fn floor_mode_without_plan_bound_ack_never_emits_motion() {
    let pty = Pty::open();
    let mut value = args();
    value.serial_path = pty.slave_path.clone();
    let _slave_guard = pty.slave_guard;
    value.motion_environment = MotionEnvironment::Floor;
    value.confirm_wheels_off_floor = false;
    let peer = thread::spawn(move || pre_observation_script(pty.master));
    let evidence = execute(&value).unwrap();
    peer.join().unwrap();
    assert!(matches!(
        evidence.outcome,
        Outcome::Refused {
            stage: "reduced_safety_authority",
            ..
        }
    ));
    assert!(evidence.required_floor_ack.is_some());
    assert!(evidence.dispatch.is_none());
}

#[test]
fn provider_loss_during_mandatory_zero_is_failed_not_safe_success() {
    let pty = Pty::open();
    let mut value = args();
    value.serial_path = pty.slave_path.clone();
    let _slave_guard = pty.slave_guard;
    let peer = thread::spawn(move || provider_loss_after_motion_script(pty.master));
    let evidence = execute(&value).unwrap();
    peer.join().unwrap();
    assert!(matches!(
        evidence.outcome,
        Outcome::Failed {
            stage: "terminal_safe_disposition",
            ..
        }
    ));
    assert!(evidence
        .terminal
        .expect("terminal report")
        .terminal
        .contains("ProviderFailure"));
    assert!(evidence.post_observation.is_none());
}

fn success_script(mut master: std::fs::File) {
    establish_safe_script(&mut master);
    observation_script(&mut master, 0);
    expect(&mut master, &[145, 0, 50, 0, 50]);
    expect(&mut master, &[145, 0, 0, 0, 0]);
    observation_script(&mut master, 12);
}

fn pre_observation_script(mut master: std::fs::File) {
    establish_safe_script(&mut master);
    observation_script(&mut master, 0);
}

fn provider_loss_after_motion_script(mut master: std::fs::File) {
    establish_safe_script(&mut master);
    observation_script(&mut master, 0);
    expect(&mut master, &[145, 0, 50, 0, 50]);
}

fn establish_safe_script(master: &mut std::fs::File) {
    expect(master, &[128]);
    expect(master, &[131]);
    expect(master, &[142, 35]);
    master.write_all(&[2]).unwrap();
}

fn observation_script(master: &mut std::fs::File, distance_delta_mm: i16) {
    expect(master, &[128]);
    expect(master, &[131]);
    expect(master, &[148, 2, 0, 34]);
    master
        .write_all(&observation_frame(distance_delta_mm))
        .unwrap();
    expect(master, &[150, 0]);
}

fn expect(master: &mut std::fs::File, expected: &[u8]) {
    let mut actual = vec![0; expected.len()];
    master.read_exact(&mut actual).unwrap();
    assert_eq!(actual, expected);
}

fn observation_frame(distance_delta_mm: i16) -> Vec<u8> {
    let mut group = [0_u8; 26];
    group[12..14].copy_from_slice(&distance_delta_mm.to_be_bytes());
    group[16] = ChargingState::NotCharging as u8;
    let mut frame = vec![19, 29, 0];
    frame.extend_from_slice(&group);
    frame.extend_from_slice(&[34, 0]);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    frame
}
