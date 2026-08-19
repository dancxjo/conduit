use super::*;
use conduit_create_oi::{
    encode_drive_direct, read_stream_packet, write_command, CreateOiFailure,
    DifferentialMotionRequest, DriveSafetySign, LocalCreateDriveSafety, MotionAuthority,
    SafetyObservation,
};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;

struct Pty {
    master: std::fs::File,
    slave_path: PathBuf,
}

impl Pty {
    fn open() -> Self {
        let mut master = -1;
        let mut slave = -1;
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0
        );
        let mut path = [0_i8; 256];
        assert_eq!(
            unsafe { libc::ttyname_r(slave, path.as_mut_ptr(), path.len()) },
            0
        );
        let path = unsafe { std::ffi::CStr::from_ptr(path.as_ptr()) };
        unsafe { libc::close(slave) };
        Self {
            master: unsafe { std::fs::File::from_raw_fd(master) },
            slave_path: PathBuf::from(path.to_str().unwrap()),
        }
    }

    fn observation(&self) -> StdCreateUartObservation {
        StdCreateUartObservation {
            base_id: "std/create-uart/0".into(),
            device_path: self.slave_path.clone(),
            profile: UartProfile::CREATE_OI,
            maximum_write_wait_ms: 100,
        }
    }
}

fn stream_frame(packet_id: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![19, payload.len() as u8 + 1, packet_id];
    frame.extend_from_slice(payload);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    frame
}

#[test]
fn exact_os_serial_base_carries_generic_create_commands_and_frames() {
    let mut pty = Pty::open();
    let mut provider = StdCreateUartBase::open(pty.observation()).unwrap();
    assert_eq!(provider.identity().base_id, "std/create-uart/0");
    assert_ne!(provider.identity().device_number, 0);

    write_command(&mut provider, &encode_drive_direct(-100, 250).unwrap()).unwrap();
    let mut command = [0_u8; 5];
    pty.master.read_exact(&mut command).unwrap();
    assert_eq!(command, [145, 0, 250, 255, 156]);

    pty.master.write_all(&stream_frame(7, &[3])).unwrap();
    let deadline = monotonic_millis().unwrap() + 500;
    let packet = read_stream_packet(&mut provider, 7, deadline).unwrap();
    assert_eq!(packet.bytes(), &[3]);

    let now = monotonic_millis().unwrap();
    let safety = SafetyObservation {
        generation: 1,
        observed_at_tick: now,
        maximum_age_ticks: 100,
        emergency_stop: false,
        wheel_drop: false,
        cliff: false,
        tilt: false,
        impact: false,
        charging: false,
        control_alive: true,
        body_link_alive: true,
        watchdog_healthy: true,
    };
    let mut drive = LocalCreateDriveSafety::new();
    assert!(matches!(
        drive.admit_motion(
            &mut provider,
            now,
            Some(MotionAuthority {
                grant_id: "test/std-drive",
                valid_until_tick: now + 1_000,
            }),
            safety,
            DifferentialMotionRequest {
                left_mm_s: 20,
                right_mm_s: 20,
                ttl_ms: 20,
            },
        ),
        DriveSafetySign::MotionAdmitted { .. }
    ));
    let mut motion = [0_u8; 5];
    pty.master.read_exact(&mut motion).unwrap();
    assert_eq!(motion, [145, 0, 20, 0, 20]);
    assert!(matches!(
        drive.supervise(
            &mut provider,
            now + 20,
            SafetyObservation {
                observed_at_tick: now + 20,
                ..safety
            },
        ),
        Some(DriveSafetySign::SafeDisposition { .. })
    ));
    let mut stopped = [0_u8; 5];
    pty.master.read_exact(&mut stopped).unwrap();
    assert_eq!(stopped, [145, 0, 0, 0, 0]);
}

#[test]
fn absent_non_character_wrong_profile_and_closed_provider_are_distinct() {
    let absent = StdCreateUartObservation {
        base_id: "std/create-uart/0".into(),
        device_path: PathBuf::from("/definitely/absent/conduit-create-uart"),
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: 100,
    };
    assert!(matches!(
        StdCreateUartBase::open(absent),
        Err(StdCreateUartOpenError::Metadata(_))
    ));

    let regular = StdCreateUartObservation {
        base_id: "std/create-uart/0".into(),
        device_path: std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: 100,
    };
    assert!(matches!(
        StdCreateUartBase::open(regular),
        Err(StdCreateUartOpenError::NotCharacterDevice)
    ));

    let pty = Pty::open();
    let mut wrong = pty.observation();
    wrong.profile.baud = 115_200;
    assert!(matches!(
        StdCreateUartBase::open(wrong),
        Err(StdCreateUartOpenError::WrongProfile(_))
    ));

    let mut unbounded = pty.observation();
    unbounded.maximum_write_wait_ms = MAXIMUM_CREATE_UART_WRITE_WAIT_MS + 1;
    assert!(matches!(
        StdCreateUartBase::open(unbounded),
        Err(StdCreateUartOpenError::InvalidWriteWait)
    ));

    let mut provider = StdCreateUartBase::open(pty.observation()).unwrap();
    provider.close();
    assert_eq!(
        write_command(&mut provider, &encode_drive_direct(0, 0).unwrap()),
        Err(CreateOiFailure::ProviderUnavailable)
    );
}

#[test]
fn no_peer_bytes_remain_device_no_response_not_base_absence() {
    let pty = Pty::open();
    let mut provider = StdCreateUartBase::open(pty.observation()).unwrap();
    let deadline = monotonic_millis().unwrap() + 10;
    assert_eq!(
        read_stream_packet(&mut provider, 7, deadline),
        Err(CreateOiFailure::DeviceNoResponse)
    );
}
