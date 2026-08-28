use conduit_core::{KindId, Quantity, QuantityUnit, QUANTITY_INFO_ID};
use conduit_human::{
    BoundKind, HumanInteractionProposal, InteractionApplicationOutcome, InteractionContract,
    InteractionCurrentState, InteractionDomain, InteractionFamily, InteractionOption,
    InteractionProposalPayload, InteractionRefusal, InteractionSelectionRules, InteractionValue,
    MutuallyExclusiveValues, OptionAvailability, RealizationRangePolicy, ScalarQuantization,
    ScalarRealizationMapping, TypedInteractionFlow,
};

const CHANNEL_KIND: &str = "audio/channel@1";

fn channel(name: &str) -> InteractionValue {
    InteractionValue::new(KindId::from(CHANNEL_KIND), name.as_bytes().to_vec()).unwrap()
}

fn option(name: &str) -> InteractionOption {
    InteractionOption {
        identity: format!("channel/{name}"),
        value: channel(name),
        availability: OptionAvailability::Available,
    }
}

fn channels_contract() -> InteractionContract {
    InteractionContract::new(
        "interaction/channels",
        InteractionFamily::ChooseMany {
            value_kind: KindId::from(CHANNEL_KIND),
            maximum_options: 4,
            minimum_selections: 1,
            maximum_selections: 3,
        },
    )
    .unwrap()
}

fn channels_state(contract: &InteractionContract) -> InteractionCurrentState {
    InteractionCurrentState::new(
        contract,
        4,
        Some(InteractionDomain {
            revision: 9,
            options: vec![option("left"), option("right"), option("center")],
        }),
        vec![channel("left")],
    )
    .unwrap()
}

fn volume_contract() -> InteractionContract {
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
    .unwrap()
}

fn decode_quantity(value: &InteractionValue) -> Quantity {
    assert_eq!(value.value_kind, KindId::from(QUANTITY_INFO_ID));
    Quantity::decode(&value.canonical_bytes).unwrap()
}

#[test]
fn executable_many_choice_flow_emits_values_and_rejects_invalid_combinations() {
    let contract = channels_contract();
    let state = channels_state(&contract);
    let rules = InteractionSelectionRules::new(
        &contract,
        vec![MutuallyExclusiveValues {
            values: vec![channel("left"), channel("right")],
        }],
    )
    .unwrap();
    let invalid = HumanInteractionProposal::new(
        &contract,
        &state,
        1,
        InteractionProposalPayload::Values(vec![channel("left"), channel("right")]),
    )
    .unwrap();
    let valid = HumanInteractionProposal::new(
        &contract,
        &state,
        2,
        InteractionProposalPayload::Values(vec![channel("center"), channel("left")]),
    )
    .unwrap();
    let mut flow = TypedInteractionFlow::new(contract, state, Some(rules), 2, 2).unwrap();
    assert_eq!(
        flow.admit(invalid),
        Err(InteractionRefusal::InvalidCombination)
    );
    flow.admit(valid.clone()).unwrap();
    let result = flow
        .finish_front(InteractionApplicationOutcome::Accepted {
            resulting_state_identity: "interaction-state/accepted".into(),
        })
        .unwrap();
    assert_eq!(result.proposal_identity, valid.proposal_identity);
}

#[test]
fn executable_single_choice_flow_carries_the_typed_value_not_an_option_index() {
    let contract = InteractionContract::new(
        "interaction/channel",
        InteractionFamily::ChooseOne {
            value_kind: KindId::from(CHANNEL_KIND),
            maximum_options: 3,
        },
    )
    .unwrap();
    let state = InteractionCurrentState::new(
        &contract,
        2,
        Some(InteractionDomain {
            revision: 8,
            options: vec![option("right"), option("left"), option("center")],
        }),
        vec![channel("left")],
    )
    .unwrap();
    let proposal = HumanInteractionProposal::new(
        &contract,
        &state,
        1,
        InteractionProposalPayload::Values(vec![channel("right")]),
    )
    .unwrap();
    assert_eq!(
        proposal.payload,
        InteractionProposalPayload::Values(vec![channel("right")])
    );
    let mut flow = TypedInteractionFlow::new(contract, state, None, 1, 1).unwrap();
    flow.admit(proposal).unwrap();
}

#[test]
fn combination_rule_identity_is_independent_of_rule_and_value_order() {
    let contract = channels_contract();
    let first = InteractionSelectionRules::new(
        &contract,
        vec![
            MutuallyExclusiveValues {
                values: vec![channel("right"), channel("left")],
            },
            MutuallyExclusiveValues {
                values: vec![channel("center"), channel("right")],
            },
        ],
    )
    .unwrap();
    let second = InteractionSelectionRules::new(
        &contract,
        vec![
            MutuallyExclusiveValues {
                values: vec![channel("right"), channel("center")],
            },
            MutuallyExclusiveValues {
                values: vec![channel("left"), channel("right")],
            },
        ],
    )
    .unwrap();
    assert_eq!(first.rules_identity, second.rules_identity);
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
}

#[test]
fn realization_mapping_keeps_source_precision_policy_and_semantic_value_explicit() {
    let contract = volume_contract();
    let nearest = ScalarRealizationMapping::new(
        &contract,
        "realization/adc-10-bit",
        0,
        1_023,
        1,
        RealizationRangePolicy::Refuse,
        ScalarQuantization::Nearest,
    )
    .unwrap();
    assert_eq!(decode_quantity(&nearest.map(0).unwrap()).value(), 0);
    assert_eq!(
        decode_quantity(&nearest.map(1_023).unwrap()).value(),
        1_000_000
    );
    assert_eq!(decode_quantity(&nearest.map(512).unwrap()).value(), 500_000);
    assert_eq!(nearest.map(1_024), Err(InteractionRefusal::OutOfRange));

    let clamped = ScalarRealizationMapping::new(
        &contract,
        "realization/adc-clamped",
        0,
        1_023,
        1,
        RealizationRangePolicy::Clamp,
        ScalarQuantization::Nearest,
    )
    .unwrap();
    assert_eq!(
        decode_quantity(&clamped.map(9_999).unwrap()).value(),
        1_000_000
    );
    assert_ne!(nearest.mapping_identity, clamped.mapping_identity);
}

#[test]
fn exact_mapping_refuses_unrepresentable_precision_instead_of_coercing() {
    let contract = volume_contract();
    let exact = ScalarRealizationMapping::new(
        &contract,
        "realization/exact-decimal",
        0,
        1_000,
        1,
        RealizationRangePolicy::Refuse,
        ScalarQuantization::Exact,
    )
    .unwrap();
    assert_eq!(decode_quantity(&exact.map(337).unwrap()).value(), 337_000);

    let inexact = ScalarRealizationMapping::new(
        &contract,
        "realization/inexact-binary",
        0,
        1_023,
        1,
        RealizationRangePolicy::Refuse,
        ScalarQuantization::Exact,
    )
    .unwrap();
    assert_eq!(
        inexact.map(512),
        Err(InteractionRefusal::UnsupportedGranularity)
    );
}

#[test]
fn bounded_flow_preserves_stale_duplicate_pressure_and_cancellation() {
    let contract = channels_contract();
    let state = channels_state(&contract);
    let proposal = HumanInteractionProposal::new(
        &contract,
        &state,
        7,
        InteractionProposalPayload::Values(vec![channel("center")]),
    )
    .unwrap();
    let next = HumanInteractionProposal::new(
        &contract,
        &state,
        8,
        InteractionProposalPayload::Values(vec![channel("right")]),
    )
    .unwrap();
    let mut flow = TypedInteractionFlow::new(contract.clone(), state.clone(), None, 1, 1).unwrap();
    flow.admit(proposal.clone()).unwrap();
    let replacement =
        InteractionCurrentState::new(&contract, 5, state.domain.clone(), vec![channel("left")])
            .unwrap();
    assert_eq!(
        flow.replace_current(replacement.clone()),
        Err(InteractionRefusal::ConcurrentStateChange)
    );
    assert_eq!(
        flow.admit(proposal),
        Err(InteractionRefusal::DuplicateProposal)
    );
    assert_eq!(flow.admit(next), Err(InteractionRefusal::QueuePressure));
    assert_eq!(
        flow.cancel_front().unwrap().outcome,
        InteractionApplicationOutcome::Cancelled
    );
    let stale = HumanInteractionProposal::new(
        &contract,
        &state,
        9,
        InteractionProposalPayload::Values(vec![channel("center")]),
    )
    .unwrap();
    flow.replace_current(replacement).unwrap();
    assert_eq!(flow.admit(stale), Err(InteractionRefusal::StaleState));
}

#[test]
fn relative_flow_keeps_delta_distinct_from_absolute_scalar() {
    let contract = InteractionContract::new(
        "interaction/transpose",
        InteractionFamily::RelativeAdjustment {
            unit: QuantityUnit::One,
            minimum_delta: -12,
            maximum_delta: 12,
            granularity: 1,
        },
    )
    .unwrap();
    let state = InteractionCurrentState::new(&contract, 0, None, vec![]).unwrap();
    let delta = InteractionValue::new(
        KindId::from(QUANTITY_INFO_ID),
        Quantity::new(-1, QuantityUnit::One).encode().to_vec(),
    )
    .unwrap();
    let proposal = HumanInteractionProposal::new(
        &contract,
        &state,
        1,
        InteractionProposalPayload::Relative(delta),
    )
    .unwrap();
    let mut flow = TypedInteractionFlow::new(contract, state, None, 1, 1).unwrap();
    flow.admit(proposal).unwrap();
}
