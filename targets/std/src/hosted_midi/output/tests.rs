use super::*;
use crate::hosted_midi::{
    HostedRawMidiSelection, MidiEndpointDirection, RawMidiEndpointObservation,
};
use conduit_core::{BootId, OfferGeneration};
use std::io::{Read, Seek, SeekFrom};

#[test]
fn raw_session_writes_exact_messages_and_cleanup_before_close() {
    let path = std::env::temp_dir().join(format!(
        "conduit-raw-midi-output-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut reader = File::options()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let writer = reader.try_clone().unwrap();
    let mut session = MidiOutputSession::prepare_test_raw(writer);
    session.send([0x90, 60, 100]).unwrap();
    session.send([0x80, 60, 0]).unwrap();
    session.stop().unwrap();
    assert_eq!(
        session.report(),
        MidiOutputReport {
            lifecycle: MidiOutputLifecycle::StoppedClosed,
            sent_messages: 3,
            all_notes_off_sent: true,
            normalized_note_events: Vec::new(),
            encoded_messages: Vec::new(),
        }
    );
    reader.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, [0x90, 60, 100, 0x80, 60, 0, 0xb0, 123, 0]);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn dropping_an_open_session_attempts_all_notes_off_before_close() {
    let path = std::env::temp_dir().join(format!(
        "conduit-raw-midi-output-drop-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let mut reader = File::options()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    {
        let writer = reader.try_clone().unwrap();
        let mut session = MidiOutputSession::prepare_test_raw(writer);
        session.send([0x90, 60, 100]).unwrap();
    }
    reader.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, [0x90, 60, 100, 0xb0, 123, 0]);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn pressure_and_provider_loss_remain_distinct() {
    assert_eq!(
        classify_write_error(&io::Error::from(io::ErrorKind::WouldBlock)),
        MidiOutputFailure::Pressure
    );
    assert_eq!(
        classify_write_error(&io::Error::from(io::ErrorKind::BrokenPipe)),
        MidiOutputFailure::ProviderLost
    );
    assert_eq!(
        classify_open_error(io::Error::from(io::ErrorKind::PermissionDenied)),
        MidiOutputFailure::BackendUnavailable
    );
}

#[test]
fn exact_but_missing_raw_device_refuses_during_pre_play_preparation() {
    let observation = RawMidiEndpointObservation {
        card: u16::MAX,
        device: u16::MAX,
        subdevice: 0,
        name: "Absent proof endpoint".into(),
        direction: MidiEndpointDirection::WritableDestination,
    };
    let selection = HostedRawMidiSelection::select(
        &[observation],
        MidiEndpointDirection::WritableDestination,
        u16::MAX,
        u16::MAX,
        0,
        BootId::from("boot-proof"),
        OfferGeneration(9),
    )
    .unwrap();
    assert_eq!(
        MidiOutputSession::prepare(MidiOutputSelection::raw(selection)).err(),
        Some(MidiOutputFailure::BackendUnavailable)
    );
}
