use conduit_core::{
    ClosingBoundary, CompatibilityOutcome, Id, InitialAvailability, ModalityReplay,
    ModalityRetention, ReplacementBehavior, SemanticHash, TemporalCardinality,
    TemporalModalityCompatibilityReason, TemporalModalityContract, TemporalModalityError,
    TemporalSurface, TypeContractRef, assess_temporal_modality_exact,
};

const ITEM: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/item"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0x61; 32]),
};

fn surfaces() -> [TemporalModalityContract<'static>; 4] {
    [
        TemporalModalityContract::value(ITEM),
        TemporalModalityContract::closing_flow(ITEM),
        TemporalModalityContract::open_flow(ITEM),
        TemporalModalityContract::current(ITEM),
    ]
}

#[test]
fn normative_fixture_covers_every_surface_and_safety_boundary() {
    let fixture = include_str!("../../../conformance/c2/temporal-modality.json");
    let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(value["suite"], "conduit.temporal-modality");
    assert_eq!(value["cases"].as_array().unwrap().len(), 10);
    for id in [
        "ordinary-value",
        "closing-flow",
        "open-flow",
        "current-observation",
        "cross-surface-matrix",
        "closing-is-not-resource-bound",
        "current-is-observation-only",
        "current-is-not-history",
        "invalid-immediate-open-flow",
        "every-field-is-identity-bearing",
    ] {
        assert!(
            value["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case["id"] == id)
        );
    }
}

#[test]
fn four_surfaces_lower_to_complete_distinct_fields() {
    let [value, closing, open, current] = surfaces();
    assert_eq!(value.surface(), Ok(TemporalSurface::Value));
    assert_eq!(closing.surface(), Ok(TemporalSurface::ClosingFlow));
    assert_eq!(open.surface(), Ok(TemporalSurface::OpenFlow));
    assert_eq!(current.surface(), Ok(TemporalSurface::Current));

    assert_eq!(value.cardinality, TemporalCardinality::ExactlyOne);
    assert_eq!(closing.cardinality, TemporalCardinality::ZeroOrMore);
    assert_eq!(closing.closing, ClosingBoundary::Available);
    assert_eq!(open.closing, ClosingBoundary::Absent);
    assert_eq!(current.initial, InitialAvailability::ImmediateCurrent);
    assert_eq!(current.retention, ModalityRetention::LatestReplacement);
    assert_eq!(current.replay, ModalityReplay::CurrentOnly);
    assert_eq!(current.replacement, ReplacementBehavior::ReplaceLatest);

    let identities = [
        value.semantic_hash().unwrap(),
        closing.semantic_hash().unwrap(),
        open.semantic_hash().unwrap(),
        current.semantic_hash().unwrap(),
    ];
    for (index, identity) in identities.iter().enumerate() {
        assert!(!identities[index + 1..].contains(identity));
    }
}

#[test]
fn every_cross_surface_connection_requires_an_explicit_conversion() {
    let modalities = surfaces();
    for (required_index, required) in modalities.iter().copied().enumerate() {
        for (candidate_index, candidate) in modalities.iter().copied().enumerate() {
            let decision = assess_temporal_modality_exact(required, candidate);
            if required_index == candidate_index {
                assert_eq!(decision.outcome, CompatibilityOutcome::Compatible);
                assert_eq!(decision.reason, TemporalModalityCompatibilityReason::Exact);
            } else {
                assert_eq!(decision.outcome, CompatibilityOutcome::Incompatible);
            }
        }
    }
}

#[test]
fn current_is_one_observable_value_plus_replacements_not_history_or_mutation() {
    let current = TemporalModalityContract::current(ITEM);
    assert_eq!(
        current.cardinality,
        TemporalCardinality::CurrentAndReplacements
    );
    assert_eq!(current.initial, InitialAvailability::ImmediateCurrent);
    assert_eq!(current.replay, ModalityReplay::CurrentOnly);
    assert_eq!(current.retention, ModalityRetention::LatestReplacement);
    assert_eq!(current.replacement, ReplacementBehavior::ReplaceLatest);
}

#[test]
fn unpublished_punctuation_combinations_fail_closed() {
    let malformed = TemporalModalityContract {
        initial: InitialAvailability::ImmediateCurrent,
        ..TemporalModalityContract::open_flow(ITEM)
    };
    assert_eq!(
        malformed.surface(),
        Err(TemporalModalityError::InvalidCombination)
    );
    assert_eq!(
        assess_temporal_modality_exact(TemporalModalityContract::open_flow(ITEM), malformed).reason,
        TemporalModalityCompatibilityReason::InvalidCandidate
    );
}

#[test]
fn closing_boundary_does_not_encode_a_resource_or_progress_bound() {
    let closing = TemporalModalityContract::closing_flow(ITEM);
    assert_eq!(closing.surface(), Ok(TemporalSurface::ClosingFlow));
    assert_eq!(closing.cardinality, TemporalCardinality::ZeroOrMore);
    assert_eq!(closing.closing, ClosingBoundary::Available);
}
