use super::*;
use conduit_midi::{MidiMessage, ParsedMidi};
use std::io::{Seek, SeekFrom, Write};

fn input_file(bytes: &[u8]) -> (File, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "conduit-raw-midi-input-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut file = File::options()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    (file, path)
}

#[test]
fn one_bounded_read_retains_order_and_one_read_timestamp() {
    let (file, path) = input_file(&[0x90, 60, 100, 0x80, 60, 0, 0xb0, 64, 127]);
    let mut session = MidiInputSession::prepare_test_raw(file);
    let expected = [
        MidiMessage::NoteOn {
            channel: 0,
            key: 60,
            velocity: 100,
        },
        MidiMessage::NoteOff {
            channel: 0,
            key: 60,
            velocity: 0,
        },
        MidiMessage::ControlChange {
            channel: 0,
            controller: 64,
            value: 127,
        },
    ];
    for (index, message) in expected.into_iter().enumerate() {
        assert_eq!(
            session.poll(100 + index as u64),
            Ok(MidiInputPoll::Observation(MidiInputObservation {
                event_time_micros: 100,
                parsed: ParsedMidi::Message(message),
            }))
        );
    }
    assert_eq!(
        session.report(),
        MidiInputReport {
            lifecycle: MidiInputLifecycle::Open,
            bytes_read: 9,
            observations: 3,
            pending_bytes: 0,
        }
    );
    assert_eq!(session.poll(103), Err(MidiInputFailure::ProviderLost));
    std::fs::remove_file(path).unwrap();
}

#[cfg(unix)]
#[test]
fn nonblocking_pressure_waits_without_consumption_then_observes_loss() {
    use std::os::fd::FromRawFd;

    let mut descriptors = [0; 2];
    // SAFETY: `pipe` initializes both descriptors on success. Each is
    // transferred exactly once into a `File` below.
    assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
    // SAFETY: the read descriptor is live and owned by this test.
    let flags = unsafe { libc::fcntl(descriptors[0], libc::F_GETFL) };
    assert!(flags >= 0);
    // SAFETY: the same live descriptor remains owned here; setting
    // O_NONBLOCK changes only its read disposition.
    assert_eq!(
        unsafe { libc::fcntl(descriptors[0], libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    // SAFETY: ownership of each fresh descriptor is transferred once.
    let reader = unsafe { File::from_raw_fd(descriptors[0]) };
    // SAFETY: ownership of the other fresh descriptor is transferred once.
    let mut writer = unsafe { File::from_raw_fd(descriptors[1]) };
    let mut session = MidiInputSession::prepare_test_raw(reader);

    assert_eq!(session.wait_readable(Duration::ZERO), Ok(false));
    assert_eq!(session.poll(10), Ok(MidiInputPoll::Pending));
    assert_eq!(session.report().bytes_read, 0);
    writer.write_all(&[0x90, 60, 100]).unwrap();
    assert_eq!(session.wait_readable(Duration::ZERO), Ok(true));
    assert!(matches!(
        session.poll(11),
        Ok(MidiInputPoll::Observation(MidiInputObservation {
            event_time_micros: 11,
            parsed: ParsedMidi::Message(MidiMessage::NoteOn { .. }),
        }))
    ));
    drop(writer);
    assert_eq!(session.wait_readable(Duration::ZERO), Ok(true));
    assert_eq!(session.poll(12), Err(MidiInputFailure::ProviderLost));
}

#[test]
fn malformed_clock_capacity_and_cancel_are_distinct() {
    let (file, path) = input_file(&[0x90, 60]);
    let mut partial = MidiInputSession::prepare_test_raw(file);
    assert_eq!(partial.poll(4), Ok(MidiInputPoll::Pending));
    assert_eq!(partial.report().pending_bytes, 1);
    assert_eq!(
        partial.poll(5),
        Err(MidiInputFailure::Malformed(
            MidiParseError::DataByteExpected(1)
        ))
    );
    std::fs::remove_file(path).unwrap();

    let (file, path) = input_file(&[0x90, 60, 0x91]);
    let mut malformed = MidiInputSession::prepare_test_raw(file);
    assert_eq!(
        malformed.poll(5),
        Err(MidiInputFailure::Malformed(
            MidiParseError::DataByteExpected(1)
        ))
    );
    std::fs::remove_file(path).unwrap();

    let (file, path) = input_file(&[0x90, 60, 1]);
    let mut clock = MidiInputSession::prepare_test_raw(file);
    assert!(matches!(clock.poll(10), Ok(MidiInputPoll::Observation(_))));
    assert_eq!(clock.poll(9), Err(MidiInputFailure::ClockRegressed));
    std::fs::remove_file(path).unwrap();

    let (file, path) = input_file(&[0x90, 60, 1]);
    let mut capacity = MidiInputSession::prepare_test_raw(file);
    capacity.bytes_read = MAXIMUM_INPUT_BYTES_PER_SESSION - 1;
    assert_eq!(capacity.poll(1), Ok(MidiInputPoll::Pending));
    assert_eq!(capacity.poll(2), Err(MidiInputFailure::CapacityExceeded));
    std::fs::remove_file(path).unwrap();

    let (file, path) = input_file(&[0x90, 60, 1]);
    let mut cancelled = MidiInputSession::prepare_test_raw(file);
    cancelled.cancel();
    assert_eq!(
        cancelled.report().lifecycle,
        MidiInputLifecycle::CancelledClosed
    );
    assert_eq!(cancelled.poll(1), Err(MidiInputFailure::InvalidLifecycle));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn exact_missing_or_wrong_direction_selection_refuses_preparation() {
    use crate::hosted_midi::RawMidiEndpointObservation;
    use conduit_core::{BootId, OfferGeneration};

    for direction in [
        MidiEndpointDirection::ReadableSource,
        MidiEndpointDirection::WritableDestination,
    ] {
        let selection = HostedRawMidiSelection::select(
            &[RawMidiEndpointObservation {
                card: u16::MAX,
                device: u16::MAX,
                subdevice: 0,
                name: "Absent input".into(),
                direction,
            }],
            direction,
            u16::MAX,
            u16::MAX,
            0,
            BootId::from("boot-input"),
            OfferGeneration(1),
        )
        .unwrap();
        assert_eq!(
            MidiInputSession::prepare(&selection).err(),
            Some(MidiInputFailure::BackendUnavailable)
        );
    }
}
