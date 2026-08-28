use conduit_core::{KindId, Quantity, QuantityUnit, QUANTITY_INFO_ID};
use conduit_human::{
    BoundKind, InteractionContract, InteractionCurrentState, InteractionDomain, InteractionFamily,
    InteractionOption, InteractionProposalPayload, InteractionValue, OptionAvailability,
    RealizationRangePolicy, ScalarQuantization, ScalarRealizationMapping,
};
use conduit_pete::{
    CalibrationProfile, ChoiceBinding, DebounceProfile, PhysicalEvent, PhysicalInput,
    PhysicalInteractionFailure, PhysicalInteractionPlanProjection, PhysicalResourceBinding,
    PhysicalResourceStatus, PicoInteractionSurface, PICO_INTERACTION_IMPLEMENTATION,
};

const WAVEFORM_KIND: &str = "music/waveform@1";

fn waveform(name: &str) -> InteractionValue {
    InteractionValue::new(KindId::from(WAVEFORM_KIND), name.as_bytes().to_vec()).unwrap()
}

fn quantity(value: i64) -> InteractionValue {
    InteractionValue::new(
        KindId::from(QUANTITY_INFO_ID),
        Quantity::new(value, QuantityUnit::Percent)
            .encode()
            .to_vec(),
    )
    .unwrap()
}

fn resource(id: &str, generation: u64) -> PhysicalResourceBinding {
    PhysicalResourceBinding {
        resource_id: id.into(),
        generation,
    }
}

fn projection() -> PhysicalInteractionPlanProjection {
    let action_contract =
        InteractionContract::new("synth/panic", InteractionFamily::Activate).unwrap();
    let action_state = InteractionCurrentState::new(&action_contract, 3, None, vec![]).unwrap();
    let choice_contract = InteractionContract::new(
        "synth/waveform",
        InteractionFamily::ChooseOne {
            value_kind: KindId::from(WAVEFORM_KIND),
            maximum_options: 3,
        },
    )
    .unwrap();
    let options = ["sine", "triangle", "saw"]
        .into_iter()
        .map(|name| InteractionOption {
            identity: format!("waveform/{name}"),
            value: waveform(name),
            availability: OptionAvailability::Available,
        })
        .collect::<Vec<_>>();
    let choice_state = InteractionCurrentState::new(
        &choice_contract,
        7,
        Some(InteractionDomain {
            revision: 2,
            options: options.clone(),
        }),
        vec![waveform("sine")],
    )
    .unwrap();
    let scalar_contract = InteractionContract::new(
        "synth/volume",
        InteractionFamily::Scalar {
            unit: QuantityUnit::Percent,
            minimum: 0,
            minimum_bound: BoundKind::Inclusive,
            maximum: 100,
            maximum_bound: BoundKind::Inclusive,
            granularity: 1,
        },
    )
    .unwrap();
    let scalar_state =
        InteractionCurrentState::new(&scalar_contract, 11, None, vec![quantity(50)]).unwrap();
    let scalar_mapping = ScalarRealizationMapping::new(
        &scalar_contract,
        "pico/adc12@1",
        200,
        3800,
        1,
        RealizationRangePolicy::Refuse,
        ScalarQuantization::Nearest,
    )
    .unwrap();
    PhysicalInteractionPlanProjection {
        plan_id: "plan/control-surface/41".into(),
        host_id: "host/pico-w/fixture".into(),
        boot_id: "boot/pico-w/93".into(),
        implementation_id: PICO_INTERACTION_IMPLEMENTATION.into(),
        action_contract,
        action_state,
        action_switch: resource("pico/gpio/10/switch", 4),
        choice_contract,
        choice_state,
        choices: options
            .into_iter()
            .enumerate()
            .map(|(index, option)| ChoiceBinding {
                resource: resource(&format!("pico/gpio/{}/switch", index + 2), 4),
                option_identity: option.identity,
                value: option.value,
            })
            .collect(),
        scalar_contract,
        scalar_state,
        scalar_resource: resource("pico/adc/0/potentiometer", 6),
        scalar_mapping,
        display_resource: resource("pico/i2c/ssd1306", 9),
        debounce: DebounceProfile {
            stable_scans: 3,
            maximum_transitions_per_window: 4,
        },
        calibration: CalibrationProfile {
            minimum_sample: 200,
            maximum_sample: 3800,
            maximum_sample_delta: 512,
        },
        maximum_pending_events: 2,
    }
}

fn event(resource_id: &str, generation: u64, sequence: u64, input: PhysicalInput) -> PhysicalEvent {
    PhysicalEvent {
        plan_id: "plan/control-surface/41".into(),
        resource_id: resource_id.into(),
        resource_generation: generation,
        sequence,
        transitions_in_window: 1,
        input,
    }
}

#[test]
fn constrained_projection_contains_only_exact_finite_assigned_truth() {
    let surface = PicoInteractionSurface::prepare(projection()).unwrap();
    let projected = surface.projection();
    assert_eq!(projected.host_id, "host/pico-w/fixture");
    assert_eq!(projected.boot_id, "boot/pico-w/93");
    assert_eq!(projected.choices.len(), 3);
    assert_eq!(projected.maximum_pending_events, 2);
    let text = format!("{projected:?}").to_ascii_lowercase();
    for forbidden in ["body", "widget", "button_number", "application_runtime"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn action_choice_and_scalar_emit_ordinary_exact_typed_proposals() {
    let mut surface = PicoInteractionSurface::prepare(projection()).unwrap();
    let action = surface
        .propose(event(
            "pico/gpio/10/switch",
            4,
            1,
            PhysicalInput::ActionPressed,
        ))
        .unwrap();
    assert_eq!(action.payload, InteractionProposalPayload::Activate);
    surface.complete_one();

    let choice = surface
        .propose(event(
            "pico/gpio/3/switch",
            4,
            2,
            PhysicalInput::ChoicePressed {
                resource_id: "pico/gpio/3/switch".into(),
            },
        ))
        .unwrap();
    assert_eq!(
        choice.payload,
        InteractionProposalPayload::Values(vec![waveform("triangle")])
    );
    assert!(!choice
        .canonical_bytes()
        .windows(6)
        .any(|bytes| bytes == b"button"));
    surface.complete_one();

    let scalar = surface
        .propose(event(
            "pico/adc/0/potentiometer",
            6,
            3,
            PhysicalInput::ScalarSample {
                sample: 2000,
                prior_sample: Some(1900),
            },
        ))
        .unwrap();
    assert_eq!(
        scalar.payload,
        InteractionProposalPayload::Values(vec![quantity(50)])
    );
}

#[test]
fn missing_resources_and_replacement_fail_distinctly_without_rewriting_semantics() {
    let original = projection();
    let original_contract = original.choice_contract.clone();
    let mut surface = PicoInteractionSurface::prepare(original).unwrap();
    assert_eq!(
        surface.propose(event(
            "pico/gpio/99/switch",
            4,
            1,
            PhysicalInput::ActionPressed
        )),
        Err(PhysicalInteractionFailure::MissingSwitch {
            resource_id: "pico/gpio/99/switch".into()
        })
    );
    assert_eq!(
        surface.propose(event(
            "pico/gpio/99/switch",
            4,
            2,
            PhysicalInput::ChoicePressed {
                resource_id: "pico/gpio/99/switch".into()
            },
        )),
        Err(PhysicalInteractionFailure::ChoiceInputUnavailable {
            resource_id: "pico/gpio/99/switch".into()
        })
    );
    assert_eq!(
        surface.propose(event(
            "pico/adc/missing",
            6,
            3,
            PhysicalInput::ScalarSample {
                sample: 2000,
                prior_sample: None
            },
        )),
        Err(PhysicalInteractionFailure::ScalarInputUnavailable {
            resource_id: "pico/adc/missing".into()
        })
    );
    assert_eq!(surface.projection().choice_contract, original_contract);

    assert_eq!(
        surface.propose(event(
            "pico/gpio/10/switch",
            3,
            4,
            PhysicalInput::ActionPressed
        )),
        Err(PhysicalInteractionFailure::OldGeneration {
            resource_id: "pico/gpio/10/switch".into(),
            expected: 4,
            observed: 3,
        })
    );
    let mut stale = event("pico/gpio/10/switch", 4, 5, PhysicalInput::ActionPressed);
    stale.plan_id = "plan/before-resource-replacement".into();
    assert!(matches!(
        surface.propose(stale),
        Err(PhysicalInteractionFailure::StalePlan { .. })
    ));
}

#[test]
fn calibration_noise_bounce_pressure_and_cancellation_stay_machine_readable() {
    let mut surface = PicoInteractionSurface::prepare(projection()).unwrap();
    assert_eq!(
        surface.propose(event(
            "pico/adc/0/potentiometer",
            6,
            1,
            PhysicalInput::ScalarSample {
                sample: 100,
                prior_sample: None
            },
        )),
        Err(PhysicalInteractionFailure::OutOfCalibration { sample: 100 })
    );
    assert_eq!(
        surface.propose(event(
            "pico/adc/0/potentiometer",
            6,
            2,
            PhysicalInput::ScalarSample {
                sample: 2000,
                prior_sample: Some(1000)
            },
        )),
        Err(PhysicalInteractionFailure::NoiseBeyondProfile {
            observed_delta: 1000
        })
    );
    let mut bouncing = event("pico/gpio/10/switch", 4, 3, PhysicalInput::ActionPressed);
    bouncing.transitions_in_window = 5;
    assert_eq!(
        surface.propose(bouncing),
        Err(PhysicalInteractionFailure::BounceBeyondProfile { transitions: 5 })
    );
    surface
        .propose(event(
            "pico/gpio/10/switch",
            4,
            4,
            PhysicalInput::ActionPressed,
        ))
        .unwrap();
    surface
        .propose(event(
            "pico/gpio/10/switch",
            4,
            5,
            PhysicalInput::ActionPressed,
        ))
        .unwrap();
    assert_eq!(
        surface.propose(event(
            "pico/gpio/10/switch",
            4,
            6,
            PhysicalInput::ActionPressed
        )),
        Err(PhysicalInteractionFailure::QueuePressure { maximum: 2 })
    );
    surface.cancel();
    assert_eq!(
        surface.propose(event(
            "pico/gpio/10/switch",
            4,
            7,
            PhysicalInput::ActionPressed
        )),
        Err(PhysicalInteractionFailure::Cancelled)
    );
}

#[test]
fn input_and_output_loss_are_independent_and_manifestation_does_not_own_state() {
    let plan = projection();
    let choice_state = plan.choice_state.clone();
    let mut surface = PicoInteractionSurface::prepare(plan).unwrap();
    assert!(matches!(
        surface.manifest(&choice_state, false),
        Err(PhysicalInteractionFailure::DisplayUnavailable { .. })
    ));
    assert!(surface
        .propose(event(
            "pico/gpio/10/switch",
            4,
            1,
            PhysicalInput::ActionPressed
        ))
        .is_ok());
    surface.complete_one();
    assert!(matches!(
        surface.propose(event(
            "pico/gpio/missing",
            4,
            2,
            PhysicalInput::ActionPressed
        )),
        Err(PhysicalInteractionFailure::MissingSwitch { .. })
    ));
    let manifestation = surface.manifest(&choice_state, true).unwrap();
    assert_eq!(manifestation.state_identity, choice_state.state_identity);
    assert_eq!(manifestation.values, vec![waveform("sine")]);
}

#[test]
fn offers_are_finite_and_resource_loss_invalidates_only_dependent_truth() {
    let surface = PicoInteractionSurface::prepare(projection()).unwrap();
    let all = PhysicalResourceStatus {
        available_resource_ids: vec![
            "pico/gpio/10/switch".into(),
            "pico/gpio/2/switch".into(),
            "pico/gpio/3/switch".into(),
            "pico/gpio/4/switch".into(),
            "pico/adc/0/potentiometer".into(),
            "pico/i2c/ssd1306".into(),
        ],
    };
    assert_eq!(
        surface.offers(&all).choice_option_identities,
        vec!["waveform/sine", "waveform/triangle", "waveform/saw"]
    );
    let without_choice_and_display = PhysicalResourceStatus {
        available_resource_ids: vec![
            "pico/gpio/10/switch".into(),
            "pico/gpio/2/switch".into(),
            "pico/gpio/4/switch".into(),
            "pico/adc/0/potentiometer".into(),
        ],
    };
    let offers = surface.offers(&without_choice_and_display);
    assert!(offers.action);
    assert!(offers.scalar);
    assert!(!offers.presentation);
    assert_eq!(
        offers.choice_option_identities,
        vec!["waveform/sine", "waveform/saw"]
    );
}
