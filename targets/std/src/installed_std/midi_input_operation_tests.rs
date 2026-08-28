use super::*;
use conduit_audio::{Gate, MusicalControl};

use conduit_kernel::ValueRef;
use conduit_midi::{MidiMessage, ParsedMidi};

fn operation() -> MidiInputOperation {
    let profile = MidiProfile::new(crate::hosted_midi::A4_REFERENCE_MILLIHERTZ, None, 0).unwrap();
    MidiInputOperation {
        adapter: MidiInputAdapter::new(profile, 1).unwrap(),
        empty_input: ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 0,
        },
        pending: None,
        next_request: 0,
        emitted: false,
    }
}

fn completion(
    operation: &mut MidiInputOperation,
    parsed: ParsedMidi,
    time: u64,
) -> OperationAction {
    let request = match operation.start() {
        OperationAction::RequestHostOperation { request, .. } => request,
        _ => panic!("MIDI source did not request an observation"),
    };
    let observation = MidiInputObservation {
        event_time_micros: time,
        parsed,
    }
    .encode()
    .unwrap();
    operation.resume_host_operation(
        request,
        HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: Some(
                BoundedValueRef::new(
                    ValueRef {
                        slot: 1,
                        generation: 1,
                        byte_len: observation.len() as u32,
                    },
                    observation.len() as u32,
                )
                .unwrap(),
            ),
            failure: None,
        },
        Some(&observation),
    )
}

fn next_completion(
    operation: &mut MidiInputOperation,
    parsed: ParsedMidi,
    time: u64,
) -> OperationAction {
    let request = match operation.advance() {
        OperationAction::RequestHostOperation { request, .. } => request,
        _ => panic!("MIDI source did not request its next observation"),
    };
    let observation = MidiInputObservation {
        event_time_micros: time,
        parsed,
    }
    .encode()
    .unwrap();
    operation.resume_host_operation(
        request,
        HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: Some(
                BoundedValueRef::new(
                    ValueRef {
                        slot: 1,
                        generation: 2,
                        byte_len: observation.len() as u32,
                    },
                    observation.len() as u32,
                )
                .unwrap(),
            ),
            failure: None,
        },
        Some(&observation),
    )
}

fn note(action: OperationAction) -> conduit_audio::MusicalNoteEvent {
    let OperationAction::EmitCanonical {
        port: PortId(0),
        value,
    } = action
    else {
        panic!("expected note output")
    };
    conduit_audio::MusicalNoteEvent::decode(value.as_slice()).unwrap()
}

fn control(action: OperationAction) -> conduit_audio::MusicalControlEvent {
    let OperationAction::EmitCanonical {
        port: PortId(1),
        value,
    } = action
    else {
        panic!("expected control output")
    };
    conduit_audio::MusicalControlEvent::decode(value.as_slice()).unwrap()
}

#[test]
fn overlap_retrigger_velocity_zero_and_order_are_deterministic() {
    let mut source = operation();
    let first = note(completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::NoteOn {
            channel: 2,
            key: 60,
            velocity: 64,
        }),
        10,
    ));
    let second = note(next_completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::NoteOn {
            channel: 2,
            key: 60,
            velocity: 127,
        }),
        11,
    ));
    let off_second = note(next_completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::NoteOn {
            channel: 2,
            key: 60,
            velocity: 0,
        }),
        12,
    ));
    let off_first = note(next_completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::NoteOff {
            channel: 2,
            key: 60,
            velocity: 9,
        }),
        13,
    ));

    assert_eq!(
        (first.occurrence.0, first.gate, first.order),
        (1, Gate::On, 0)
    );
    assert_eq!(
        (second.occurrence.0, second.gate, second.order),
        (2, Gate::On, 1)
    );
    assert_eq!((off_second.occurrence.0, off_second.gate), (2, Gate::Off));
    assert_eq!((off_first.occurrence.0, off_first.gate), (1, Gate::Off));
    assert!(second.velocity > first.velocity);
}

#[test]
fn sustain_modulation_and_pitch_bend_use_the_control_port() {
    let mut source = operation();
    let sustain = control(completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::ControlChange {
            channel: 0,
            controller: 64,
            value: 127,
        }),
        20,
    ));
    let modulation = control(next_completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::ControlChange {
            channel: 0,
            controller: 1,
            value: 96,
        }),
        21,
    ));
    let bend = control(next_completion(
        &mut source,
        ParsedMidi::Message(MidiMessage::PitchBend {
            channel: 0,
            value: 16_383,
        }),
        22,
    ));
    assert_eq!(sustain.control, MusicalControl::Sustain { down: true });
    assert!(matches!(
        modulation.control,
        MusicalControl::Modulation { .. }
    ));
    assert!(matches!(bend.control, MusicalControl::PitchBend { .. }));
    assert_eq!((sustain.order, modulation.order, bend.order), (0, 1, 2));
}

#[test]
fn unsupported_protocol_observation_and_cancellation_fail_closed() {
    let mut source = operation();
    assert!(matches!(
        completion(&mut source, ParsedMidi::UnsupportedSysEx { bytes: 3 }, 30),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 95
        })
    ));
    source.cancel();
    assert_eq!(source.adapter.active_notes(), 0);
    assert!(source.pending.is_none());
}
