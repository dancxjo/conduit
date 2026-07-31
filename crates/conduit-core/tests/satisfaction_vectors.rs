use conduit_core::{
    CompatibilityClass, CompatibilityDecision, CompatibilityOutcome, CompatibilityQuery,
    CompatibilityReason, ConnectionCardinality, Delivery, DescriptorRef, Direction,
    ExplicitSatisfactionRequirement, Id, LossAcceptance, PortContract, PortFlowConstraints,
    Presence, SatisfactionCandidate, SatisfactionFacet, SatisfactionMethod, SatisfactionObligation,
    SatisfactionPin, SatisfactionProof, SatisfactionProofError, SatisfactionReason,
    SatisfactionRole, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality, assess_port_connection, select_satisfaction_candidate,
    validate_port_satisfaction_proof, validate_satisfaction_proof,
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn descriptor(kind: &'static str, byte: u8) -> DescriptorRef<'static> {
    DescriptorRef {
        kind: Id(kind),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}

fn required_ids(role: SatisfactionRole) -> &'static [&'static str] {
    match role {
        SatisfactionRole::PortConnection | SatisfactionRole::PortSubstitution => &[
            "direction",
            "semantic-type",
            "presence",
            "connection-cardinality",
            "value-cardinality",
            "delivery",
            "temporal",
            "terminal",
            "sensitivity",
            "authority",
            "representation",
            "ownership-lifetime",
            "flow",
            "boundedness",
        ],
        SatisfactionRole::Implementation => &[
            "semantic-contract",
            "ports",
            "configuration",
            "representation",
            "ownership-lifetime",
            "lifecycle",
            "authority",
            "resources",
            "boundedness",
        ],
        SatisfactionRole::HostCapability => &[
            "semantic-capability",
            "observation-freshness",
            "resources",
            "effects",
            "authority",
            "boundedness",
        ],
    }
}

fn obligations(
    role: SatisfactionRole,
    failure: Option<(&str, CompatibilityOutcome)>,
) -> Vec<SatisfactionObligation<'static>> {
    required_ids(role)
        .iter()
        .enumerate()
        .map(|(index, id)| SatisfactionObligation {
            id: Id(id),
            required_hash: hash(40 + index as u8),
            offered_hash: hash(40 + index as u8),
            outcome: failure
                .filter(|(failed, _)| failed == id)
                .map_or(CompatibilityOutcome::Compatible, |(_, outcome)| outcome),
            reason: failure
                .filter(|(failed, _)| failed == id)
                .map_or(Id("fixture/accepted"), |_| Id("fixture/rejected")),
        })
        .collect()
}

fn proof_with<'a>(
    role: SatisfactionRole,
    required: DescriptorRef<'a>,
    offered: DescriptorRef<'a>,
    obligations: &'a [SatisfactionObligation<'a>],
    facets: &'a [SatisfactionFacet<'a>],
    result: (
        CompatibilityOutcome,
        SatisfactionReason,
        ExplicitSatisfactionRequirement<'a>,
    ),
) -> SatisfactionProof<'a> {
    let (outcome, reason, explicit_requirement) = result;
    let provider = SatisfactionPin {
        descriptor: DescriptorRef {
            kind: Id("fixture/provider"),
            schema_version: 0,
            semantic_hash: hash(9),
        },
    };
    let provider_available = reason != SatisfactionReason::ProviderUnavailable;
    let mut proof = SatisfactionProof {
        schema_version: 0,
        identity: hash(0),
        role,
        method: SatisfactionMethod::StructuralFacets,
        required,
        offered,
        provider: provider_available.then_some(provider),
        provider_rule: provider_available.then_some(Id("fixture/structural-v1")),
        policy: None,
        facets,
        obligations,
        outcome,
        reason,
        explanation: Id("fixture/explanation"),
        explicit_requirement,
    };
    let mut scratch = vec![hash(0); proof.identity_fact_count()];
    proof.identity = proof.semantic_hash(&mut scratch).unwrap();
    proof
}

fn accepted_proof<'a>(
    role: SatisfactionRole,
    required: DescriptorRef<'a>,
    offered: DescriptorRef<'a>,
    obligations: &'a [SatisfactionObligation<'a>],
    facets: &'a [SatisfactionFacet<'a>],
) -> SatisfactionProof<'a> {
    proof_with(
        role,
        required,
        offered,
        obligations,
        facets,
        (
            CompatibilityOutcome::Compatible,
            SatisfactionReason::Satisfied,
            ExplicitSatisfactionRequirement::None,
        ),
    )
}

#[test]
fn required_fixture_inventory_is_frozen() {
    let fixture = include_str!("../../../conformance/c2/implicit-satisfaction.json");
    let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(value["suite"], "conduit.implicit-satisfaction");
    assert_eq!(value["cases"].as_array().unwrap().len(), 24);
    for required in [
        "directional-structural-port-success",
        "exact-nominal-success",
        "same-shape-different-semantics-rejected",
        "missing-provider-indeterminate",
        "stale-host-report-indeterminate",
        "deterministic-policy-selection",
        "unresolved-candidate-ambiguity",
        "explicit-adapter-required",
        "ownership-lifetime-violation",
        "distinct-host-realizations",
        "shuffled-facets-and-obligations",
        "proof-identity-mutation-rejected",
        "proof-operand-omission-rejected",
        "source-identity-stable-across-realizations",
    ] {
        assert!(
            value["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case["id"] == required)
        );
    }
}

#[test]
fn structural_port_proof_is_complete_directional_and_order_independent() {
    let required = descriptor("conduit/port-contract", 1);
    let offered = descriptor("conduit/port-contract", 2);
    let obligations = obligations(SatisfactionRole::PortConnection, None);
    let facets = [
        SatisfactionFacet {
            id: Id("audio/bounds"),
            required_hash: hash(3),
            offered_hash: hash(3),
        },
        SatisfactionFacet {
            id: Id("audio/meaning"),
            required_hash: hash(4),
            offered_hash: hash(4),
        },
    ];
    let proof = accepted_proof(
        SatisfactionRole::PortConnection,
        required,
        offered,
        &obligations,
        &facets,
    );
    let mut scratch = vec![hash(0); proof.identity_fact_count()];
    assert_eq!(validate_satisfaction_proof(&proof, &mut scratch), Ok(()));

    let mut reversed_obligations = obligations.clone();
    reversed_obligations.reverse();
    let reversed_facets = [facets[1], facets[0]];
    let reordered = accepted_proof(
        SatisfactionRole::PortConnection,
        required,
        offered,
        &reversed_obligations,
        &reversed_facets,
    );
    assert_eq!(proof.identity, reordered.identity);
}

#[test]
fn port_proof_reuses_the_frozen_complete_port_decision() {
    let value_type = TypeContractRef {
        contract_id: Id("fixture/value"),
        schema_version: 0,
        semantic_hash: hash(30),
    };
    let port = |id, direction| PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::OneOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Restricted,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    };
    let consumer = port("in", Direction::Input);
    let producer = port("out", Direction::Output);
    let type_decision = CompatibilityDecision::compatible(
        CompatibilityQuery::ConsumerAcceptsProducer {
            consumer: value_type,
            producer: value_type,
        },
        CompatibilityClass::Exact,
        CompatibilityReason::TypeContractExact,
        None,
    );
    let port_decision = assess_port_connection(consumer, producer, type_decision);
    let mut obligations = obligations(SatisfactionRole::PortConnection, None);
    let type_obligation = obligations
        .iter_mut()
        .find(|obligation| obligation.id == Id("semantic-type"))
        .unwrap();
    type_obligation.required_hash = value_type.semantic_hash;
    type_obligation.offered_hash = value_type.semantic_hash;
    let facets = [SatisfactionFacet {
        id: Id("fixture/complete-port"),
        required_hash: hash(31),
        offered_hash: hash(31),
    }];
    let proof = accepted_proof(
        SatisfactionRole::PortConnection,
        DescriptorRef {
            kind: Id("conduit/port-contract"),
            schema_version: 0,
            semantic_hash: consumer.semantic_hash().unwrap(),
        },
        DescriptorRef {
            kind: Id("conduit/port-contract"),
            schema_version: 0,
            semantic_hash: producer.semantic_hash().unwrap(),
        },
        &obligations,
        &facets,
    );
    let mut scratch = vec![hash(0); proof.identity_fact_count()];
    assert_eq!(
        validate_port_satisfaction_proof(&proof, port_decision, &mut scratch),
        Ok(())
    );

    let mut unrelated = proof;
    unrelated.offered.semantic_hash = hash(32);
    unrelated.identity = unrelated.semantic_hash(&mut scratch).unwrap();
    assert_eq!(
        validate_port_satisfaction_proof(&unrelated, port_decision, &mut scratch),
        Err(SatisfactionProofError::CompatibilityMismatch)
    );
}

#[test]
fn exact_nominal_success_needs_no_provider_or_shape() {
    let exact = descriptor("fixture/nominal", 7);
    let mut proof = SatisfactionProof {
        schema_version: 0,
        identity: hash(0),
        role: SatisfactionRole::Implementation,
        method: SatisfactionMethod::ExactNominal,
        required: exact,
        offered: exact,
        provider: None,
        provider_rule: None,
        policy: None,
        facets: &[],
        obligations: &[],
        outcome: CompatibilityOutcome::Compatible,
        reason: SatisfactionReason::Satisfied,
        explanation: Id("fixture/exact"),
        explicit_requirement: ExplicitSatisfactionRequirement::None,
    };
    proof.identity = proof.semantic_hash(&mut []).unwrap();
    assert_eq!(validate_satisfaction_proof(&proof, &mut []), Ok(()));
}

#[test]
fn semantic_failures_and_indeterminate_observations_are_not_permissive() {
    for (failed, reason, outcome) in [
        (
            "sensitivity",
            SatisfactionReason::ObligationRejected,
            CompatibilityOutcome::Incompatible,
        ),
        (
            "ownership-lifetime",
            SatisfactionReason::ObligationRejected,
            CompatibilityOutcome::Incompatible,
        ),
        (
            "delivery",
            SatisfactionReason::ObligationRejected,
            CompatibilityOutcome::Incompatible,
        ),
        (
            "temporal",
            SatisfactionReason::ObligationRejected,
            CompatibilityOutcome::Incompatible,
        ),
        (
            "value-cardinality",
            SatisfactionReason::ObligationRejected,
            CompatibilityOutcome::Incompatible,
        ),
        (
            "semantic-type",
            SatisfactionReason::UnsupportedFacet,
            CompatibilityOutcome::Incompatible,
        ),
        (
            "semantic-type",
            SatisfactionReason::ProviderUnavailable,
            CompatibilityOutcome::Indeterminate,
        ),
        (
            "semantic-type",
            SatisfactionReason::ProviderStale,
            CompatibilityOutcome::Indeterminate,
        ),
    ] {
        let obligations = obligations(SatisfactionRole::PortConnection, Some((failed, outcome)));
        let facet_offered = if reason == SatisfactionReason::UnsupportedFacet {
            hash(9)
        } else {
            hash(8)
        };
        let facets = [SatisfactionFacet {
            id: Id("fixture/complete"),
            required_hash: hash(8),
            offered_hash: facet_offered,
        }];
        let proof = proof_with(
            SatisfactionRole::PortConnection,
            descriptor("conduit/port-contract", 1),
            descriptor("conduit/port-contract", 2),
            &obligations,
            &facets,
            (outcome, reason, ExplicitSatisfactionRequirement::None),
        );
        let mut scratch = vec![hash(0); proof.identity_fact_count()];
        assert_eq!(validate_satisfaction_proof(&proof, &mut scratch), Ok(()));
    }
}

#[test]
fn explicit_adapter_is_named_but_never_applied() {
    let obligations = obligations(
        SatisfactionRole::PortConnection,
        Some(("representation", CompatibilityOutcome::Incompatible)),
    );
    let facets = [SatisfactionFacet {
        id: Id("fixture/representation"),
        required_hash: hash(6),
        offered_hash: hash(7),
    }];
    let adapter = descriptor("fixture/adapter", 33);
    let proof = proof_with(
        SatisfactionRole::PortConnection,
        descriptor("conduit/port-contract", 1),
        descriptor("conduit/port-contract", 2),
        &obligations,
        &facets,
        (
            CompatibilityOutcome::Incompatible,
            SatisfactionReason::ExplicitAdapterRequired,
            ExplicitSatisfactionRequirement::Adapter(adapter),
        ),
    );
    let mut scratch = vec![hash(0); proof.identity_fact_count()];
    assert_eq!(validate_satisfaction_proof(&proof, &mut scratch), Ok(()));
    assert_eq!(
        proof.explicit_requirement,
        ExplicitSatisfactionRequirement::Adapter(adapter)
    );
}

#[test]
fn host_realizations_share_requirements_but_keep_distinct_proofs() {
    let required = descriptor("fixture/can-host-network", 1);
    let linux_obligations = obligations(SatisfactionRole::HostCapability, None);
    let pico_obligations = obligations(SatisfactionRole::HostCapability, None);
    let facets = [SatisfactionFacet {
        id: Id("network/wifi"),
        required_hash: hash(21),
        offered_hash: hash(21),
    }];
    let linux = accepted_proof(
        SatisfactionRole::HostCapability,
        required,
        descriptor("fixture/linux-report", 2),
        &linux_obligations,
        &facets,
    );
    let pico = accepted_proof(
        SatisfactionRole::HostCapability,
        required,
        descriptor("fixture/pico-report", 3),
        &pico_obligations,
        &facets,
    );
    assert_eq!(linux.required, pico.required);
    assert_ne!(linux.offered, pico.offered);
    assert_ne!(linux.identity, pico.identity);
}

#[test]
fn deterministic_policy_does_not_depend_on_candidate_order() {
    let required = descriptor("fixture/contract", 1);
    let first_obligations = obligations(SatisfactionRole::Implementation, None);
    let second_obligations = obligations(SatisfactionRole::Implementation, None);
    let facets = [SatisfactionFacet {
        id: Id("fixture/facet"),
        required_hash: hash(4),
        offered_hash: hash(4),
    }];
    let first = accepted_proof(
        SatisfactionRole::Implementation,
        required,
        descriptor("fixture/first", 2),
        &first_obligations,
        &facets,
    );
    let second = accepted_proof(
        SatisfactionRole::Implementation,
        required,
        descriptor("fixture/second", 3),
        &second_obligations,
        &facets,
    );
    let policy = SatisfactionPin {
        descriptor: descriptor("fixture/lowest-rank-policy", 10),
    };
    let candidates = [
        SatisfactionCandidate {
            id: Id("second"),
            proof: &second,
            policy_rank: 20,
        },
        SatisfactionCandidate {
            id: Id("first"),
            proof: &first,
            policy_rank: 10,
        },
    ];
    let reversed = [candidates[1], candidates[0]];
    let selected = select_satisfaction_candidate(&candidates, Some(policy));
    let selected_reversed = select_satisfaction_candidate(&reversed, Some(policy));
    assert_eq!(selected.selected, Some(Id("first")));
    assert_eq!(selected, selected_reversed);

    let ambiguous = select_satisfaction_candidate(&candidates, None);
    assert_eq!(ambiguous.outcome, CompatibilityOutcome::Indeterminate);
    assert_eq!(ambiguous.reason, SatisfactionReason::Ambiguous);
}

#[test]
fn mutation_and_omission_are_rejected() {
    let obligations = obligations(SatisfactionRole::Implementation, None);
    let facets = [SatisfactionFacet {
        id: Id("fixture/facet"),
        required_hash: hash(4),
        offered_hash: hash(4),
    }];
    let proof = accepted_proof(
        SatisfactionRole::Implementation,
        descriptor("fixture/contract", 1),
        descriptor("fixture/implementation", 2),
        &obligations,
        &facets,
    );
    let mut mutated = proof;
    mutated.offered.semantic_hash = hash(99);
    let mut scratch = vec![hash(0); mutated.identity_fact_count()];
    assert_eq!(
        validate_satisfaction_proof(&mutated, &mut scratch),
        Err(SatisfactionProofError::IdentityMismatch)
    );

    let incomplete = &obligations[..obligations.len() - 1];
    let omitted = proof_with(
        SatisfactionRole::Implementation,
        descriptor("fixture/contract", 1),
        descriptor("fixture/implementation", 2),
        incomplete,
        &facets,
        (
            CompatibilityOutcome::Compatible,
            SatisfactionReason::Satisfied,
            ExplicitSatisfactionRequirement::None,
        ),
    );
    let mut scratch = vec![hash(0); omitted.identity_fact_count()];
    assert_eq!(
        validate_satisfaction_proof(&omitted, &mut scratch),
        Err(SatisfactionProofError::MissingObligation)
    );
}
