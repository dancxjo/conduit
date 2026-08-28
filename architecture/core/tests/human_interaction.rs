use conduit_core::{
    BoundKind, HumanInteractionProposal, InfoBool, InteractionApplicationOutcome,
    InteractionContract, InteractionCurrentState, InteractionDomain, InteractionFamily,
    InteractionOption, InteractionProposalPayload, InteractionProposalQueue, InteractionRefusal,
    InteractionValue, KindId, OptionAvailability, Quantity, QuantityUnit, StructuredInfoType,
    StructuredInfoValue, BOOL_INFO_ID, QUANTITY_INFO_ID, TEXT_INFO_ID,
};

const WAVEFORM_KIND: &str = "music/waveform@1";

fn value(kind: &str, bytes: &[u8]) -> InteractionValue {
    InteractionValue::new(KindId::from(kind), bytes.to_vec()).unwrap()
}

fn quantity(value: i64, unit: QuantityUnit) -> InteractionValue {
    InteractionValue::new(
        KindId::from(QUANTITY_INFO_ID),
        Quantity::new(value, unit).encode().to_vec(),
    )
    .unwrap()
}

fn option(identity: &str, bytes: &[u8]) -> InteractionOption {
    InteractionOption {
        identity: identity.into(),
        value: value(WAVEFORM_KIND, bytes),
        availability: OptionAvailability::Available,
    }
}

fn waveform_contract() -> InteractionContract {
    InteractionContract::new(
        "interaction/waveform",
        InteractionFamily::ChooseOne {
            value_kind: KindId::from(WAVEFORM_KIND),
            maximum_options: 4,
        },
    )
    .unwrap()
}

fn waveform_domain() -> InteractionDomain {
    InteractionDomain {
        revision: 7,
        options: vec![
            option("waveform/sine", b"sine"),
            option("waveform/triangle", b"triangle"),
            option("waveform/saw", b"saw"),
            option("waveform/pulse", b"pulse"),
        ],
    }
}

#[test]
fn one_portable_algebra_covers_every_family_without_renderer_or_device_vocabulary() {
    let structured_type = StructuredInfoType::leaf(KindId::from("domain/reviewed-leaf@1")).unwrap();
    let structured_profile = structured_type.profile().unwrap();
    let contracts = [
        InteractionContract::new("interaction/activate", InteractionFamily::Activate).unwrap(),
        InteractionContract::new("interaction/bool", InteractionFamily::Boolean).unwrap(),
        waveform_contract(),
        InteractionContract::new(
            "interaction/channels",
            InteractionFamily::ChooseMany {
                value_kind: KindId::from("audio/channel@1"),
                maximum_options: 16,
                minimum_selections: 0,
                maximum_selections: 8,
            },
        )
        .unwrap(),
        InteractionContract::new(
            "interaction/volume",
            InteractionFamily::Scalar {
                unit: QuantityUnit::Millionth,
                minimum: 0,
                minimum_bound: BoundKind::Inclusive,
                maximum: 1_000_000,
                maximum_bound: BoundKind::Inclusive,
                granularity: 1_000,
            },
        )
        .unwrap(),
        InteractionContract::new(
            "interaction/transpose",
            InteractionFamily::RelativeAdjustment {
                unit: QuantityUnit::One,
                minimum_delta: -24,
                maximum_delta: 24,
                granularity: 1,
            },
        )
        .unwrap(),
        InteractionContract::new(
            "interaction/text",
            InteractionFamily::Text {
                maximum_bytes: 4_096,
                allow_empty: false,
            },
        )
        .unwrap(),
        InteractionContract::new(
            "interaction/structured",
            InteractionFamily::Structured {
                value_kind: structured_profile.value_kind().clone(),
                type_digest: structured_type.semantic_digest().unwrap(),
                maximum_bytes: 4_096,
            },
        )
        .unwrap(),
    ];
    assert_eq!(contracts.len(), 8);
    for contract in contracts {
        let encoded = contract.canonical_bytes();
        assert!(!encoded.is_empty());
        let debug = format!("{contract:?}").to_ascii_lowercase();
        for forbidden in ["dom", "widget", "gpio", "hid", "midi", "renderer"] {
            assert!(!debug.contains(forbidden), "contract leaked {forbidden}");
        }
    }
}

#[test]
fn action_boolean_absolute_and_relative_semantics_remain_distinct() {
    let activate = InteractionContract::new("interaction/go", InteractionFamily::Activate).unwrap();
    let boolean =
        InteractionContract::new("interaction/enabled", InteractionFamily::Boolean).unwrap();
    assert_ne!(activate.contract_identity, boolean.contract_identity);
    assert!(InteractionCurrentState::new(&activate, 0, None, vec![]).is_ok());
    assert_eq!(
        InteractionCurrentState::new(
            &activate,
            0,
            None,
            vec![value(BOOL_INFO_ID, &InfoBool::TRUE.encode())]
        ),
        Err(InteractionRefusal::InvalidCurrentState)
    );

    let absolute = InteractionContract::new(
        "interaction/cutoff",
        InteractionFamily::Scalar {
            unit: QuantityUnit::Hertz,
            minimum: 20,
            minimum_bound: BoundKind::Inclusive,
            maximum: 20_000,
            maximum_bound: BoundKind::Exclusive,
            granularity: 5,
        },
    )
    .unwrap();
    let relative = InteractionContract::new(
        "interaction/cutoff-adjust",
        InteractionFamily::RelativeAdjustment {
            unit: QuantityUnit::Hertz,
            minimum_delta: -100,
            maximum_delta: 100,
            granularity: 5,
        },
    )
    .unwrap();
    let absolute_state =
        InteractionCurrentState::new(&absolute, 1, None, vec![quantity(440, QuantityUnit::Hertz)])
            .unwrap();
    let relative_state = InteractionCurrentState::new(&relative, 1, None, vec![]).unwrap();
    assert!(HumanInteractionProposal::new(
        &absolute,
        &absolute_state,
        0,
        InteractionProposalPayload::Values(vec![quantity(445, QuantityUnit::Hertz)])
    )
    .is_ok());
    assert!(HumanInteractionProposal::new(
        &relative,
        &relative_state,
        0,
        InteractionProposalPayload::Relative(quantity(5, QuantityUnit::Hertz))
    )
    .is_ok());
    assert_eq!(
        HumanInteractionProposal::new(
            &relative,
            &relative_state,
            0,
            InteractionProposalPayload::Values(vec![quantity(5, QuantityUnit::Hertz)])
        ),
        Err(InteractionRefusal::WrongValueKind)
    );
}

#[test]
fn choice_identity_is_typed_stable_and_independent_of_order_or_labels() {
    let contract = waveform_contract();
    let domain = waveform_domain();
    let state = InteractionCurrentState::new(
        &contract,
        9,
        Some(domain.clone()),
        vec![value(WAVEFORM_KIND, b"sine")],
    )
    .unwrap();
    let mut reordered = domain;
    reordered.options.reverse();
    let reordered_state = InteractionCurrentState::new(
        &contract,
        9,
        Some(reordered),
        vec![value(WAVEFORM_KIND, b"sine")],
    )
    .unwrap();
    assert_eq!(state.state_identity, reordered_state.state_identity);
    assert_eq!(state.canonical_bytes(), reordered_state.canonical_bytes());
    assert!(!format!("{state:?}").contains("Sine wave"));
}

#[test]
fn structured_and_bounded_text_values_use_ordinary_canonical_info() {
    let value_type = StructuredInfoType::leaf(KindId::from("domain/exact-code@1")).unwrap();
    let structured = StructuredInfoValue::leaf(value_type.clone(), b"alpha".to_vec()).unwrap();
    let interaction_value = InteractionValue::structured(&structured).unwrap();
    assert_eq!(
        interaction_value.canonical_bytes,
        structured.canonical_bytes().unwrap()
    );
    let contract = InteractionContract::new(
        "interaction/structured",
        InteractionFamily::Structured {
            value_kind: interaction_value.value_kind.clone(),
            type_digest: value_type.semantic_digest().unwrap(),
            maximum_bytes: 1_024,
        },
    )
    .unwrap();
    let state = InteractionCurrentState::new(&contract, 0, None, vec![]).unwrap();
    assert!(HumanInteractionProposal::new(
        &contract,
        &state,
        0,
        InteractionProposalPayload::Values(vec![interaction_value])
    )
    .is_ok());

    let text = InteractionContract::new(
        "interaction/message",
        InteractionFamily::Text {
            maximum_bytes: 8,
            allow_empty: false,
        },
    )
    .unwrap();
    let text_state = InteractionCurrentState::new(&text, 0, None, vec![]).unwrap();
    assert!(HumanInteractionProposal::new(
        &text,
        &text_state,
        0,
        InteractionProposalPayload::Values(vec![value(TEXT_INFO_ID, b"hello")])
    )
    .is_ok());
    assert_eq!(
        HumanInteractionProposal::new(
            &text,
            &text_state,
            1,
            InteractionProposalPayload::Values(vec![value(TEXT_INFO_ID, &[0xff])])
        ),
        Err(InteractionRefusal::MalformedValue)
    );
}

#[test]
fn stale_removed_unavailable_wrong_type_range_and_granularity_refuse_distinctly() {
    let contract = waveform_contract();
    let state =
        InteractionCurrentState::new(&contract, 7, Some(waveform_domain()), vec![]).unwrap();
    let proposal = HumanInteractionProposal::new(
        &contract,
        &state,
        1,
        InteractionProposalPayload::Values(vec![value(WAVEFORM_KIND, b"sine")]),
    )
    .unwrap();
    let fresh =
        InteractionCurrentState::new(&contract, 8, Some(waveform_domain()), vec![]).unwrap();
    assert_eq!(
        proposal.validate_against(&contract, &fresh),
        Err(InteractionRefusal::StaleState)
    );
    assert_eq!(
        HumanInteractionProposal::new(
            &contract,
            &state,
            2,
            InteractionProposalPayload::Values(vec![value(WAVEFORM_KIND, b"noise")])
        ),
        Err(InteractionRefusal::RemovedOption)
    );
    let mut unavailable_domain = waveform_domain();
    unavailable_domain.options[0].availability = OptionAvailability::Unavailable {
        reason_code: "not-now".into(),
    };
    let unavailable = InteractionCurrentState::new(
        &contract,
        7,
        Some(unavailable_domain),
        vec![value(WAVEFORM_KIND, b"sine")],
    )
    .unwrap();
    assert_eq!(
        HumanInteractionProposal::new(
            &contract,
            &unavailable,
            3,
            InteractionProposalPayload::Values(vec![value(WAVEFORM_KIND, b"sine")])
        ),
        Err(InteractionRefusal::UnavailableOption)
    );
    assert_eq!(
        HumanInteractionProposal::new(
            &contract,
            &state,
            4,
            InteractionProposalPayload::Values(vec![value("wrong/kind@1", b"sine")])
        ),
        Err(InteractionRefusal::WrongValueKind)
    );

    let scalar = InteractionContract::new(
        "interaction/volume",
        InteractionFamily::Scalar {
            unit: QuantityUnit::Percent,
            minimum: 0,
            minimum_bound: BoundKind::Inclusive,
            maximum: 100,
            maximum_bound: BoundKind::Inclusive,
            granularity: 5,
        },
    )
    .unwrap();
    let scalar_state =
        InteractionCurrentState::new(&scalar, 0, None, vec![quantity(50, QuantityUnit::Percent)])
            .unwrap();
    assert_eq!(
        HumanInteractionProposal::new(
            &scalar,
            &scalar_state,
            0,
            InteractionProposalPayload::Values(vec![quantity(101, QuantityUnit::Percent)])
        ),
        Err(InteractionRefusal::OutOfRange)
    );
    assert_eq!(
        HumanInteractionProposal::new(
            &scalar,
            &scalar_state,
            1,
            InteractionProposalPayload::Values(vec![quantity(52, QuantityUnit::Percent)])
        ),
        Err(InteractionRefusal::UnsupportedGranularity)
    );
}

#[test]
fn proposal_queue_keeps_duplicate_pressure_cancellation_and_result_identity_distinct() {
    let contract = InteractionContract::new("interaction/go", InteractionFamily::Activate).unwrap();
    let state = InteractionCurrentState::new(&contract, 0, None, vec![]).unwrap();
    let first =
        HumanInteractionProposal::new(&contract, &state, 1, InteractionProposalPayload::Activate)
            .unwrap();
    let second =
        HumanInteractionProposal::new(&contract, &state, 2, InteractionProposalPayload::Activate)
            .unwrap();
    assert_ne!(first.proposal_identity, state.state_identity);
    let mut queue = InteractionProposalQueue::new(1, 1).unwrap();
    queue.admit(first.clone()).unwrap();
    assert_eq!(
        queue.admit(first.clone()),
        Err(InteractionRefusal::DuplicateProposal)
    );
    assert_eq!(queue.admit(second), Err(InteractionRefusal::QueuePressure));
    let cancelled = queue.cancel_front().unwrap();
    assert_eq!(cancelled.proposal_identity, first.proposal_identity);
    assert_eq!(cancelled.outcome, InteractionApplicationOutcome::Cancelled);
    assert_ne!(cancelled.result_identity, cancelled.proposal_identity);
    assert_eq!(
        queue.admit(first),
        Err(InteractionRefusal::DuplicateProposal)
    );
    assert_eq!(queue.queued_len(), 0);
    let third =
        HumanInteractionProposal::new(&contract, &state, 3, InteractionProposalPayload::Activate)
            .unwrap();
    queue.admit(third).unwrap();
    assert_eq!(
        queue.cancel_front(),
        Err(InteractionRefusal::ResultPressure)
    );
    assert_eq!(queue.queued_len(), 1);
}

#[test]
fn many_choice_identity_treats_selection_order_as_non_semantic() {
    let contract = InteractionContract::new(
        "interaction/layers",
        InteractionFamily::ChooseMany {
            value_kind: KindId::new(WAVEFORM_KIND),
            maximum_options: 4,
            minimum_selections: 1,
            maximum_selections: 2,
        },
    )
    .unwrap();
    let domain = waveform_domain();
    let a = value(WAVEFORM_KIND, b"triangle");
    let b = value(WAVEFORM_KIND, b"pulse");
    let state_ab = InteractionCurrentState::new(
        &contract,
        1,
        Some(domain.clone()),
        vec![a.clone(), b.clone()],
    )
    .unwrap();
    let state_ba =
        InteractionCurrentState::new(&contract, 1, Some(domain), vec![b.clone(), a.clone()])
            .unwrap();
    assert_eq!(state_ab.state_identity, state_ba.state_identity);
    let proposal_ab = HumanInteractionProposal::new(
        &contract,
        &state_ab,
        1,
        InteractionProposalPayload::Values(vec![a, b.clone()]),
    )
    .unwrap();
    let proposal_ba = HumanInteractionProposal::new(
        &contract,
        &state_ab,
        1,
        InteractionProposalPayload::Values(vec![b, value(WAVEFORM_KIND, b"triangle")]),
    )
    .unwrap();
    assert_eq!(proposal_ab.proposal_identity, proposal_ba.proposal_identity);
}

#[test]
fn canonical_contract_state_proposal_and_result_vectors_are_deterministic() {
    let contract = waveform_contract();
    let state = InteractionCurrentState::new(
        &contract,
        9,
        Some(waveform_domain()),
        vec![value(WAVEFORM_KIND, b"triangle")],
    )
    .unwrap();
    let proposal = HumanInteractionProposal::new(
        &contract,
        &state,
        12,
        InteractionProposalPayload::Values(vec![value(WAVEFORM_KIND, b"pulse")]),
    )
    .unwrap();
    let result = conduit_core::InteractionApplicationResult::new(
        &proposal,
        InteractionApplicationOutcome::Accepted {
            resulting_state_identity: "interaction-state/result".into(),
        },
    )
    .unwrap();
    assert_eq!(
        contract.canonical_bytes(),
        waveform_contract().canonical_bytes()
    );
    assert_eq!(
        contract.contract_identity,
        "interaction-contract/27b5d9240dc1dbe675e13d5e8b12bc30137b26ce29753d09054afb982f96d427"
    );
    for identity in [
        &contract.contract_identity,
        &state.state_identity,
        &proposal.proposal_identity,
        &result.result_identity,
    ] {
        assert_eq!(identity.rsplit('/').next().unwrap().len(), 64);
    }
    assert_ne!(state.state_identity, proposal.proposal_identity);
    assert_ne!(proposal.proposal_identity, result.result_identity);
}
