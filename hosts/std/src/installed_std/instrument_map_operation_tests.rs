use super::*;
use conduit_core::{
    kind_id, BootId, ConfigurationEntry, GearId, HostId, OfferGeneration, PlacementId,
    StructuredFieldValue, StructuredInfoType,
};

fn placement() -> PlannedGear {
    let offer = conduit_std_offers::instrument_map_std_offer();
    PlannedGear {
        placement_id: PlacementId::from("instrument-map-placement"),
        gear_id: GearId::from("mapper"),
        kind_id: offer.kind_id,
        kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry {
            key: "mapping".into(),
            value: ConfigurationValue::Structured(
                conduit_std_catalog::default_instrument_mapping_configuration().unwrap(),
            ),
        }],
        host_id: HostId::from("instrument-host"),
        boot_id: BootId::from("instrument-boot"),
        offer_generation: OfferGeneration(1),
        capability_id: offer.capability_id,
        implementation_id: offer.implementation.implementation_id,
        artifact_id: offer.implementation.artifact_id,
        realization_characteristics: Vec::new(),
        limits: offer.limits,
        inputs: offer.inputs,
        outputs: offer.outputs,
        host_operations: offer.host_operations,
        resources: Vec::new(),
        authority: Vec::new(),
        pool_references: Vec::new(),
    }
}

fn test_mapping() -> InstrumentMapping {
    InstrumentMapping {
        pitch_millihertz: [
            261_626, 293_665, 329_628, 349_228, 391_995, 440_000, 493_883, 523_251,
        ],
        sustain_button: 8,
        modulation_control: 0,
        expression_control: 1,
    }
}

fn leaf(kind: &str, value: impl ToString) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(kind)).unwrap(),
        value.to_string().into_bytes(),
    )
    .unwrap()
}

fn control(tag: &str, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    let control_type = conduit_std_catalog::instrument_control_type();
    let payload_type = match control_type.shape() {
        conduit_core::StructuredInfoTypeShape::Variant { cases, .. } => cases
            .iter()
            .find(|case| case.tag() == tag)
            .unwrap()
            .payload_type()
            .clone(),
        _ => unreachable!(),
    };
    let payload = StructuredInfoValue::record(
        payload_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap();
    StructuredInfoValue::variant(control_type, tag, payload).unwrap()
}

fn button(index: u64, down: bool, occurrence: u64, time: u64) -> StructuredInfoValue {
    control(
        "button",
        vec![
            ("index", leaf("value/count@1", index)),
            ("down", leaf("value/boolean@1", down)),
            ("occurrence", leaf("value/count@1", occurrence)),
            ("event_time_micros", leaf("value/count@1", time)),
        ],
    )
}

fn analog(index: u64, value: u64, time: u64) -> StructuredInfoValue {
    control(
        "analog",
        vec![
            ("index", leaf("value/count@1", index)),
            ("value", leaf("value/count@1", value)),
            ("event_time_micros", leaf("value/count@1", time)),
        ],
    )
}

fn note(action: OperationAction) -> MusicalNoteEvent {
    let OperationAction::EmitCanonical {
        port: PortId(0),
        value,
    } = action
    else {
        panic!("expected exact note output")
    };
    MusicalNoteEvent::decode(value.as_slice()).unwrap()
}

fn musical_control(action: OperationAction) -> MusicalControlEvent {
    let OperationAction::EmitCanonical {
        port: PortId(1),
        value,
    } = action
    else {
        panic!("expected exact control output")
    };
    MusicalControlEvent::decode(value.as_slice()).unwrap()
}

#[test]
fn eight_buttons_map_to_portable_frequencies_and_preserve_identity_and_time() {
    let mapping = test_mapping();
    for (index, frequency) in mapping.pitch_millihertz.into_iter().enumerate() {
        let value = button(
            index as u64,
            index % 2 == 0,
            index as u64 + 1,
            10 + index as u64,
        );
        let event = note(map_control(&mapping, &value, index as u32).unwrap());
        assert_eq!(event.pitch.frequency_millihertz, frequency);
        assert_eq!(event.occurrence, NoteOccurrenceId(index as u64 + 1));
        assert_eq!(event.event_time_micros, 10 + index as u64);
        assert_eq!(event.order, index as u32);
        assert_eq!(
            event.gate,
            if index % 2 == 0 { Gate::On } else { Gate::Off }
        );
    }
}

#[test]
fn sustain_modulation_and_expression_use_the_distinct_control_port() {
    let mapping = test_mapping();
    let sustain = musical_control(map_control(&mapping, &button(8, true, 9, 20), 0).unwrap());
    assert_eq!(sustain.control, MusicalControl::Sustain { down: true });
    assert_eq!(sustain.event_time_micros, 20);

    let modulation = musical_control(map_control(&mapping, &analog(0, 250_000, 21), 1).unwrap());
    assert_eq!(
        modulation.control,
        MusicalControl::Modulation {
            amount_millionths: 250_000,
            destination: ModulationDestination::FilterCutoff,
        }
    );
    let expression = musical_control(map_control(&mapping, &analog(1, 750_000, 22), 2).unwrap());
    assert_eq!(
        expression.control,
        MusicalControl::Modulation {
            amount_millionths: 750_000,
            destination: ModulationDestination::Amplitude,
        }
    );
}

#[test]
fn invalid_indices_values_and_occurrences_are_refused() {
    let mapping = test_mapping();
    assert!(matches!(
        map_control(&mapping, &button(9, true, 1, 0), 0),
        Err(187)
    ));
    assert!(matches!(
        map_control(&mapping, &button(0, true, 0, 0), 0),
        Err(189)
    ));
    assert!(matches!(
        map_control(&mapping, &analog(2, 0, 0), 0),
        Err(197)
    ));
    assert!(matches!(
        map_control(&mapping, &analog(0, 1_000_001, 0), 0),
        Err(194)
    ));
}

#[test]
fn operation_waits_for_pressure_release_and_only_closes_when_idle() {
    let mut operation = InstrumentMapOperation {
        mapping: test_mapping(),
        next_order: 0,
        emitted: false,
    };
    let canonical = button(0, true, 1, 10).canonical_bytes().unwrap();
    assert!(matches!(operation.start(), OperationAction::Await));
    assert!(matches!(
        operation.resume_value(PortId(0), &canonical),
        OperationAction::EmitCanonical {
            port: PortId(0),
            ..
        }
    ));
    assert!(matches!(
        operation.resume_value(PortId(0), &canonical),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert!(matches!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert!(matches!(operation.advance(), OperationAction::Await));
    assert!(matches!(
        operation.resume(OperationInput::Closed { port: PortId(0) }),
        OperationAction::Complete
    ));
}

#[test]
fn wrong_profile_and_malformed_canonical_input_fail_closed() {
    let mut operation = InstrumentMapOperation {
        mapping: test_mapping(),
        next_order: 0,
        emitted: false,
    };
    let wrong = leaf("value/count@1", 1).canonical_bytes().unwrap();
    assert!(matches!(
        operation.resume_value(PortId(0), &wrong),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 176
        })
    ));
    assert!(matches!(
        operation.resume_value(PortId(0), &[0xff]),
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 175
        })
    ));
}

#[test]
fn admitted_event_bound_fails_closed_before_consuming_more_input() {
    let mut operation = InstrumentMapOperation {
        mapping: test_mapping(),
        next_order: u32::from(conduit_std_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS),
        emitted: false,
    };
    let canonical = button(0, true, 1, 10).canonical_bytes().unwrap();
    assert!(matches!(
        operation.resume_value(PortId(0), &canonical),
        OperationAction::Fail(Failure {
            code: FailureCode::StorageExhausted,
            detail: 172
        })
    ));
}

#[test]
fn factory_accepts_only_the_exact_planned_offer_and_structured_mapping() {
    let exact = placement();
    assert!(budget(&exact).is_ok());
    let mut values = conduit_kernel::HostedValueStore::new(4, 1024, 4096).unwrap();
    assert!(matches!(
        prepare(&exact, &mut values),
        Ok(InstalledOperation::InstrumentMap(_))
    ));

    let mut wrong_identity = exact.clone();
    wrong_identity.artifact_id = conduit_core::ArtifactId::from("wrong/artifact");
    assert!(validate(&wrong_identity).is_err());

    let mut wrong_mapping = exact;
    wrong_mapping.configuration[0].key = "other".into();
    assert!(super::mapping(&wrong_mapping).is_err());
}
