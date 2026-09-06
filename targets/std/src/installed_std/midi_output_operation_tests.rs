use super::*;
use conduit_audio::{Gate, NoteOccurrenceId};
use conduit_core::{
    AuthorityBinding, AuthorityGrantId, BootId, GearId, HostId, OfferGeneration, PlacementId,
    ResourceBinding,
};

fn fixture() -> (PlannedGear, crate::hosted_midi::HostedMidiSelection) {
    let host_id = HostId::from("midi-host");
    let boot_id = BootId::from("midi-boot");
    let generation = OfferGeneration(7);
    let observation = crate::hosted_midi::MidiEndpointObservation {
        client: 20,
        port: 1,
        client_name: "Loopback".into(),
        port_name: "Output".into(),
        client_type: "user".into(),
        direction: crate::hosted_midi::MidiEndpointDirection::WritableDestination,
    };
    let selection = crate::hosted_midi::HostedMidiSelection::select(
        &[observation],
        crate::hosted_midi::MidiEndpointDirection::WritableDestination,
        20,
        1,
        boot_id.clone(),
        generation,
    )
    .unwrap()
    .with_fake_output(crate::hosted_midi::output_fake::FakeMidiOutputBehavior::Healthy);
    let offer = conduit_std_offers::music_play_midi_offer();
    let advertisement = selection
        .output_realization_advertisement(host_id.clone())
        .unwrap();
    let resource = ResourceBinding {
        content: None,
        pool_id: selection.resource_pool_id(),
        class_id: conduit_core::ResourceClassId::from(
            conduit_std_offers::MIDI_OUTPUT_RESOURCE_CLASS,
        ),
        units: 1,
        protected: None,
        compute: None,
    };
    let authority = offer
        .authority_requirements
        .iter()
        .enumerate()
        .map(|(index, requirement)| AuthorityBinding {
            grant_id: AuthorityGrantId::from(format!("midi-grant-{index}")),
            contract_id: requirement.contract_id.clone(),
            host_operation_contract_id: requirement.host_operation_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            capability_id: offer.capability_id.clone(),
        })
        .collect();
    (
        PlannedGear {
            placement_id: PlacementId::from("midi-placement"),
            gear_id: GearId::from("midi-output"),
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            execution_profile_id: offer.implementation.execution_profile_id,
            configuration: Vec::new(),
            host_id,
            boot_id,
            offer_generation: generation,
            capability_id: offer.capability_id,
            implementation_id: offer.implementation.implementation_id,
            artifact_id: offer.implementation.artifact_id,
            realization_characteristics: advertisement.characteristics,
            limits: offer.limits,
            inputs: offer.inputs,
            outputs: offer.outputs,
            host_operations: offer.host_operations,
            resources: vec![resource],
            authority,
            pool_references: Vec::new(),
        },
        selection,
    )
}

#[test]
fn exact_portable_events_cross_the_bounded_host_boundary_in_order() {
    let (placement, selection) = fixture();
    let selected = crate::hosted_midi::MidiOutputSelection::sequencer(selection);
    let mut session = prepare_session(&placement, Some(&selected)).unwrap();
    let mut adapter = prepare_adapter().unwrap();
    let pitch = conduit_audio::MusicalPitch::from_equal_tempered(
        0,
        crate::hosted_midi::A4_REFERENCE_MILLIHERTZ,
        0,
    )
    .unwrap();
    let on =
        conduit_audio::MusicalNoteEvent::new(NoteOccurrenceId(9), pitch, Gate::On, u16::MAX, 10, 0)
            .unwrap();
    let sustain = conduit_audio::MusicalControlEvent::new(
        conduit_audio::MusicalControl::Sustain { down: true },
        11,
        1,
    )
    .unwrap();
    let off = conduit_audio::MusicalNoteEvent::new(NoteOccurrenceId(9), pitch, Gate::Off, 0, 12, 2)
        .unwrap();
    for (contract, encoded) in [
        (
            conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION,
            on.encode().to_vec(),
        ),
        (
            conduit_std_offers::MUSIC_PLAY_MIDI_CONTROL_OPERATION,
            sustain.encode().to_vec(),
        ),
        (
            conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION,
            off.encode().to_vec(),
        ),
    ] {
        assert_eq!(
            execute(&mut adapter, &mut session, contract, &encoded).disposition,
            HostOperationDisposition::Completed
        );
    }
    session.stop().unwrap();
    let report = session.report();
    assert_eq!(
        report.encoded_messages,
        vec![
            [0x90, 69, 127],
            [0xb0, 64, 127],
            [0x80, 69, 0],
            [0xb0, 123, 0]
        ]
    );
    assert!(report.all_notes_off_sent);
}

#[test]
fn stale_authority_and_unrepresentable_pitch_fail_closed() {
    let (mut placement, selection) = fixture();
    placement.authority.pop();
    let selected = crate::hosted_midi::MidiOutputSelection::sequencer(selection);
    assert!(prepare_session(&placement, Some(&selected)).is_err());

    let (placement, selection) = fixture();
    let selected = crate::hosted_midi::MidiOutputSelection::sequencer(selection);
    let mut session = prepare_session(&placement, Some(&selected)).unwrap();
    let mut adapter = prepare_adapter().unwrap();
    let pitch = conduit_audio::MusicalPitch::from_equal_tempered(
        0,
        crate::hosted_midi::A4_REFERENCE_MILLIHERTZ,
        1,
    )
    .unwrap();
    let event =
        conduit_audio::MusicalNoteEvent::new(NoteOccurrenceId(1), pitch, Gate::On, u16::MAX, 0, 0)
            .unwrap();
    let outcome = execute(
        &mut adapter,
        &mut session,
        conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION,
        &event.encode(),
    );
    assert_eq!(outcome.disposition, HostOperationDisposition::Failed);
    assert_eq!(
        outcome.failure.unwrap().code,
        conduit_kernel::FailureCode::InvalidInput
    );
    assert!(session.report().encoded_messages.is_empty());
}

#[test]
fn provider_loss_remains_a_host_failure() {
    let (placement, selection) = fixture();
    let selection = selection
        .with_fake_output(crate::hosted_midi::output_fake::FakeMidiOutputBehavior::FailAfter(0));
    let selected = crate::hosted_midi::MidiOutputSelection::sequencer(selection);
    let mut session = prepare_session(&placement, Some(&selected)).unwrap();
    let mut adapter = prepare_adapter().unwrap();
    let pitch = conduit_audio::MusicalPitch::from_equal_tempered(
        0,
        crate::hosted_midi::A4_REFERENCE_MILLIHERTZ,
        0,
    )
    .unwrap();
    let event =
        conduit_audio::MusicalNoteEvent::new(NoteOccurrenceId(2), pitch, Gate::On, u16::MAX, 0, 0)
            .unwrap();
    let outcome = execute(
        &mut adapter,
        &mut session,
        conduit_std_offers::MUSIC_PLAY_MIDI_NOTE_OPERATION,
        &event.encode(),
    );
    assert_eq!(outcome.disposition, HostOperationDisposition::Failed);
    assert_eq!(
        outcome.failure.unwrap().code,
        conduit_kernel::FailureCode::HostOperationFailed
    );
    assert_eq!(
        session.report().lifecycle,
        crate::hosted_midi::MidiOutputLifecycle::Failed
    );
    assert!(session.report().normalized_note_events.is_empty());
}

#[test]
fn scheduler_operation_keeps_note_and_control_bindings_distinct() {
    let mut operation = MidiOutputOperation {
        pending: None,
        next_request: 0,
        closed: [false; 2],
    };
    let note_value = conduit_kernel::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: conduit_audio::NOTE_EVENT_ENCODED_LEN as u32,
    };
    let action = operation.resume(OperationInput::Value {
        port: PortId(0),
        value: note_value,
    });
    assert!(matches!(
        action,
        OperationAction::RequestHostOperation {
            request: RequestId(0),
            operation: HostOperationId(1),
            ..
        }
    ));
    assert_eq!(
        operation.resume(OperationInput::HostOperationCompleted {
            request: RequestId(0),
            outcome: completed(),
        }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Await
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(1) }),
        OperationAction::Complete
    );
}
