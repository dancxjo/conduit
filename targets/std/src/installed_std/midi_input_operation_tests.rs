use super::*;
use conduit_audio::{Gate, MusicalControl};

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    ValueRef,
};
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
) -> Result<(PortId, Vec<u8>), StepOutcome> {
    let mut request_io = StepIo::test_frame(
        [None; 2],
        [false; 2],
        [
            Some(conduit_audio::NOTE_EVENT_ENCODED_LEN as u32),
            Some(conduit_audio::CONTROL_EVENT_ENCODED_LEN as u32),
        ],
        None,
        8,
    );
    assert_eq!(
        operation.step(
            &mut request_io,
            &StepInputBytes::test_frame([None; 2], None)
        ),
        StepOutcome::Progress
    );
    let request = request_io
        .test_host_request()
        .expect("MIDI source requested an observation")
        .0;
    let observation = MidiInputObservation {
        event_time_micros: time,
        parsed,
    }
    .encode()
    .unwrap();
    let output = BoundedValueRef::new(
        ValueRef {
            slot: 1,
            generation: u16::try_from(request.0).unwrap() + 1,
            byte_len: observation.len() as u32,
        },
        observation.len() as u32,
    )
    .unwrap();
    let mut io = StepIo::test_frame(
        [None; 2],
        [false; 2],
        [
            Some(conduit_audio::NOTE_EVENT_ENCODED_LEN as u32),
            Some(conduit_audio::CONTROL_EVENT_ENCODED_LEN as u32),
        ],
        Some((
            request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(output),
                failure: None,
            },
        )),
        8,
    );
    let outcome = operation.step(
        &mut io,
        &StepInputBytes::test_frame([None; 2], Some(&observation)),
    );
    match outcome {
        StepOutcome::Progress => {
            let (port, value) = io
                .test_canonical_output()
                .expect("MIDI completion emitted a portable event");
            Ok((*port, value.as_slice().to_vec()))
        }
        outcome => Err(outcome),
    }
}

fn next_completion(
    operation: &mut MidiInputOperation,
    parsed: ParsedMidi,
    time: u64,
) -> Result<(PortId, Vec<u8>), StepOutcome> {
    completion(operation, parsed, time)
}

fn note(output: Result<(PortId, Vec<u8>), StepOutcome>) -> conduit_audio::MusicalNoteEvent {
    let (port, value) = output.expect("note completion succeeds");
    assert_eq!(port, PortId(0));
    conduit_audio::MusicalNoteEvent::decode(&value).unwrap()
}

fn control(output: Result<(PortId, Vec<u8>), StepOutcome>) -> conduit_audio::MusicalControlEvent {
    let (port, value) = output.expect("control completion succeeds");
    assert_eq!(port, PortId(1));
    conduit_audio::MusicalControlEvent::decode(&value).unwrap()
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
    assert_eq!(
        completion(&mut source, ParsedMidi::UnsupportedSysEx { bytes: 3 }, 30),
        Err(StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 95
        }))
    );
    StepBack::<2>::cancel(&mut source);
    assert_eq!(source.adapter.active_notes(), 0);
    assert!(source.pending.is_none());
}
