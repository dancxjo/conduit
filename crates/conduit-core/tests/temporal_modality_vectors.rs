use conduit_core::{
    ClosingBoundary, CompatibilityOutcome, DescriptorRef, Id, InitialAvailability, LiftedSurfaces,
    ModalityLiftContract, ModalityLiftContractError, ModalityLiftError, ModalityReplay,
    ModalityRetention, ReplacementBehavior, SemanticHash, TemporalCardinality,
    TemporalModalityCompatibilityReason, TemporalModalityContract, TemporalModalityError,
    TemporalSurface, TypeContractRef, assess_temporal_modality_exact, lift_temporal_modality,
};

const ITEM: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/item"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0x61; 32]),
};

const OUTPUT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/output"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0x62; 32]),
};

fn descriptor(kind: &'static str, byte: u8) -> DescriptorRef<'static> {
    DescriptorRef {
        kind: Id(kind),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

fn lift_contract(admitted: LiftedSurfaces) -> ModalityLiftContract<'static> {
    ModalityLiftContract {
        node_contract: descriptor("conduit/node-contract", 0x71),
        receiving_port: Id("input"),
        outgoing_port: Id("output"),
        input_type: ITEM,
        output_type: OUTPUT,
        admitted,
        purity_proof: descriptor("conduit/purity-proof", 0x72),
        law_proof: descriptor("conduit/modality-lift-law-proof", 0x73),
    }
}

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
    assert_eq!(value["cases"].as_array().unwrap().len(), 20);
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
        "explicit-pure-lift",
        "unadmitted-current-lift",
        "lift-input-type-mismatch",
        "lift-proof-required",
        "lift-declaration-is-identity-bearing",
        "current-late-subscriber",
        "current-update-without-subscriber",
        "current-reconnect-newest",
        "current-denied-mutation",
        "current-equal-replacement",
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

#[test]
fn explicit_lift_changes_only_item_type_for_each_admitted_surface() {
    let contract = lift_contract(LiftedSurfaces::ALL);
    for input in surfaces() {
        let output = lift_temporal_modality(contract, input).unwrap();
        assert_eq!(output.item_type, OUTPUT);
        assert_eq!(output.cardinality, input.cardinality);
        assert_eq!(output.closing, input.closing);
        assert_eq!(output.initial, input.initial);
        assert_eq!(output.retention, input.retention);
        assert_eq!(output.replay, input.replay);
        assert_eq!(output.replacement, input.replacement);
        assert_eq!(output.surface(), input.surface());
    }
}

#[test]
fn lift_is_never_inferred_for_an_unadmitted_surface_or_type() {
    let contract = lift_contract(LiftedSurfaces {
        value: true,
        closing_flow: true,
        open_flow: true,
        current: false,
    });
    assert_eq!(
        lift_temporal_modality(contract, TemporalModalityContract::current(ITEM)),
        Err(ModalityLiftError::SurfaceNotAdmitted(
            TemporalSurface::Current
        ))
    );
    assert_eq!(
        lift_temporal_modality(contract, TemporalModalityContract::value(OUTPUT)),
        Err(ModalityLiftError::InputTypeMismatch)
    );
}

#[test]
fn lift_requires_independent_purity_and_law_proof_identities() {
    let valid = lift_contract(LiftedSurfaces::ALL);
    assert_eq!(valid.validate(), Ok(()));

    let invalid = ModalityLiftContract {
        purity_proof: descriptor("not-namespaced", 0x72),
        ..valid
    };
    assert_eq!(
        invalid.validate(),
        Err(ModalityLiftContractError::InvalidPurityProof)
    );

    let no_surfaces = ModalityLiftContract {
        admitted: LiftedSurfaces::NONE,
        ..valid
    };
    assert_eq!(
        no_surfaces.validate(),
        Err(ModalityLiftContractError::NoAdmittedSurface)
    );
}

#[test]
fn every_lift_declaration_fact_is_identity_bearing() {
    let contract = lift_contract(LiftedSurfaces::ALL);
    let changed = ModalityLiftContract {
        law_proof: descriptor("conduit/modality-lift-law-proof", 0x74),
        ..contract
    };
    assert_ne!(
        contract.semantic_hash().unwrap(),
        changed.semantic_hash().unwrap()
    );
}
