use conduit_core::{
    CompatibilityClass, CompatibilityDecision, CompatibilityOutcome, CompatibilityQuery,
    CompatibilityReason, ConfigContract, ConnectionCardinality, Delivery, DescriptorRef, Direction,
    Id, InterfaceMemberRequirement, LossAcceptance, NodeContract, NodeInterfaceContract,
    NodeInterfaceContractError, NodeInterfaceContractRef, NodeInterfaceIdentityError,
    NodeInterfaceMember, NodeInterfaceMemberProof, NodeInterfaceMemberReason,
    NodeInterfaceRequirement, NodeInterfaceRequirementDecision, NodeInterfaceRequirementProof,
    NodeInterfaceRequirementReason, NodeInterfaceSatisfactionError,
    NodeInterfaceSatisfactionReason, NodeInterfaceTypeDecision, PortCompatibilityReason,
    PortContract, PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract,
    TerminalContract, TypeContractRef, ValueCardinality, assess_node_interface,
};

const VALUE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([0x11; 32]),
};

const OTHER_VALUE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/other-value"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([0x22; 32]),
};

fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

fn port(id: &'static str, direction: Direction) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type: VALUE,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Restricted,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

fn member(
    id: &'static str,
    direction: Direction,
    requirement: InterfaceMemberRequirement,
) -> NodeInterfaceMember<'static> {
    NodeInterfaceMember {
        requirement,
        port: port(id, direction),
    }
}

fn placeholder_proof() -> NodeInterfaceMemberProof<'static> {
    NodeInterfaceMemberProof {
        required: member(
            "placeholder",
            Direction::Input,
            InterfaceMemberRequirement::Required,
        ),
        offered: None,
        type_decision: None,
        port_decision: None,
        outcome: CompatibilityOutcome::Indeterminate,
        reason: NodeInterfaceMemberReason::IndeterminatePort,
    }
}

fn interface_reference<'a>(interface: &NodeInterfaceContract<'a>) -> NodeInterfaceContractRef<'a> {
    let mut scratch = vec![hash(0); interface.members.len() + interface.requirements.len()];
    NodeInterfaceContractRef {
        contract_id: interface.id,
        schema_version: 1,
        semantic_hash: interface.semantic_hash(&mut scratch).unwrap(),
    }
}

fn placeholder_requirement_proof() -> NodeInterfaceRequirementProof<'static> {
    NodeInterfaceRequirementProof {
        required: NodeInterfaceRequirement {
            id: Id("conduit/placeholder"),
            contract: DescriptorRef {
                kind: Id("fixture/placeholder"),
                schema_version: 1,
                semantic_hash: hash(0),
            },
        },
        offered: None,
        decision: None,
        outcome: CompatibilityOutcome::Indeterminate,
        reason: NodeInterfaceRequirementReason::FactUnavailable,
    }
}

fn candidate_reference(byte: u8) -> DescriptorRef<'static> {
    DescriptorRef {
        kind: Id("conduit/node-contract"),
        schema_version: 2,
        semantic_hash: hash(byte),
    }
}

#[test]
fn language_neutral_fixture_inventory_is_frozen() {
    let fixture = include_str!("../../../conformance/c2/node-interface-v1.json");
    let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(value["suite"], "conduit.node-interface/v1");
    assert_eq!(value["portable_limits"]["maximum_members"], 64);
    assert_eq!(value["cases"].as_array().unwrap().len(), 21);
    for required in [
        "exact-primitive-boundary",
        "compatible-directional-refinement",
        "composite-exported-boundary",
        "optional-member-absent",
        "optional-member-present-but-incompatible",
        "wrong-direction",
        "provider-unavailable",
        "ambiguous-provider-decisions",
        "authority-effect-widening-rejected",
        "revision-or-hash-mismatch",
        "insufficient-proof-scratch",
        "claim-does-not-prove-deferred-facets",
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
fn required_non_port_facts_are_directional_and_cannot_widen_authority() {
    let required_authority = DescriptorRef {
        kind: Id("conduit/node-effect-contract"),
        schema_version: 1,
        semantic_hash: hash(0x31),
    };
    let requirements = [NodeInterfaceRequirement {
        id: Id("conduit/authority-effects"),
        contract: required_authority,
    }];
    let interface = NodeInterfaceContract {
        id: Id("fixture/authority-bounded"),
        members: &[],
        requirements: &requirements,
    };
    let interface_ref = interface_reference(&interface);
    let candidate = NodeContract {
        id: Id("fixture/effectful-node"),
        config: ConfigContract { fields: &[] },
        inputs: &[],
        outputs: &[],
    };
    let candidate_authority = DescriptorRef {
        semantic_hash: hash(0x32),
        ..required_authority
    };
    let incompatible_decision = NodeInterfaceRequirementDecision {
        requirement_id: Id("conduit/authority-effects"),
        decision: CompatibilityDecision::incompatible(
            CompatibilityQuery::CandidateSubstitutesRequired {
                required: required_authority,
                candidate: candidate_authority,
            },
            CompatibilityReason::SemanticHashMismatch,
            None,
        ),
    };
    let mut requirement_scratch = [placeholder_requirement_proof()];
    let mut identity_hashes = [hash(0)];
    let mut proof_hashes = [hash(0)];
    let incompatible = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(0x33),
        &candidate,
        &[],
        &[incompatible_decision],
        &mut [],
        &mut requirement_scratch,
        &mut identity_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(incompatible.outcome, CompatibilityOutcome::Incompatible);
    assert_eq!(
        incompatible.reason,
        NodeInterfaceSatisfactionReason::IncompatibleRequirement
    );
    assert_eq!(
        incompatible.requirements[0].reason,
        NodeInterfaceRequirementReason::Incompatible
    );

    let missing = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(0x33),
        &candidate,
        &[],
        &[],
        &mut [],
        &mut requirement_scratch,
        &mut identity_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(missing.outcome, CompatibilityOutcome::Indeterminate);
    assert_eq!(
        missing.reason,
        NodeInterfaceSatisfactionReason::MissingRequirementFact
    );

    let exact_decision = NodeInterfaceRequirementDecision {
        requirement_id: Id("conduit/authority-effects"),
        decision: CompatibilityDecision::compatible(
            CompatibilityQuery::CandidateSubstitutesRequired {
                required: required_authority,
                candidate: required_authority,
            },
            CompatibilityClass::Substitutable,
            CompatibilityReason::ExactIdentity,
            None,
        ),
    };
    let compatible = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(0x33),
        &candidate,
        &[],
        &[exact_decision],
        &mut [],
        &mut requirement_scratch,
        &mut identity_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(compatible.outcome, CompatibilityOutcome::Compatible);
}

#[test]
fn interface_identity_is_namespaced_order_independent_and_semantic() {
    let members = [
        member(
            "audio",
            Direction::Input,
            InterfaceMemberRequirement::Required,
        ),
        member(
            "final",
            Direction::Output,
            InterfaceMemberRequirement::Required,
        ),
    ];
    let reversed = [members[1], members[0]];
    let interface = NodeInterfaceContract {
        id: Id("speech/recognizer"),
        members: &members,
        requirements: &[],
    };
    let reordered = NodeInterfaceContract {
        members: &reversed,
        ..interface
    };
    let mut first_scratch = [hash(0); 2];
    let mut second_scratch = [hash(0); 2];
    assert_eq!(
        interface.semantic_hash(&mut first_scratch).unwrap(),
        reordered.semantic_hash(&mut second_scratch).unwrap()
    );

    let changed_members = [
        members[0],
        NodeInterfaceMember {
            requirement: InterfaceMemberRequirement::Optional,
            ..members[1]
        },
    ];
    let changed = NodeInterfaceContract {
        members: &changed_members,
        ..interface
    };
    assert_ne!(
        interface.semantic_hash(&mut first_scratch).unwrap(),
        changed.semantic_hash(&mut second_scratch).unwrap()
    );

    let local = NodeInterfaceContract {
        id: Id("recognizer"),
        ..interface
    };
    assert_eq!(
        local.validate(),
        Err(NodeInterfaceContractError::MissingNamespace(Id(
            "recognizer"
        )))
    );
    assert_eq!(
        interface.semantic_hash(&mut []),
        Err(NodeInterfaceIdentityError::ScratchTooSmall)
    );
}

#[test]
fn exact_primitive_and_composite_boundaries_use_the_same_proof_path() {
    let members = [
        member(
            "audio",
            Direction::Input,
            InterfaceMemberRequirement::Required,
        ),
        member(
            "final",
            Direction::Output,
            InterfaceMemberRequirement::Required,
        ),
    ];
    let interface = NodeInterfaceContract {
        id: Id("speech/recognizer"),
        members: &members,
        requirements: &[],
    };
    let interface_ref = interface_reference(&interface);
    let inputs = [port("audio", Direction::Input)];
    let outputs = [port("final", Direction::Output)];
    let primitive = NodeContract {
        id: Id("fixture/primitive-recognizer"),
        config: ConfigContract { fields: &[] },
        inputs: &inputs,
        outputs: &outputs,
    };
    let composite = NodeContract {
        id: Id("fixture/composite-recognizer"),
        ..primitive
    };
    let mut primitive_members = vec![placeholder_proof(); members.len()];
    let mut composite_members = vec![placeholder_proof(); members.len()];
    let mut primitive_interface_hashes = [hash(0); 2];
    let mut composite_interface_hashes = [hash(0); 2];
    let mut primitive_proof_hashes = [hash(0); 2];
    let mut composite_proof_hashes = [hash(0); 2];
    let primitive_proof = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(1),
        &primitive,
        &[],
        &[],
        &mut primitive_members,
        &mut [],
        &mut primitive_interface_hashes,
        &mut primitive_proof_hashes,
    )
    .unwrap();
    let composite_proof = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(2),
        &composite,
        &[],
        &[],
        &mut composite_members,
        &mut [],
        &mut composite_interface_hashes,
        &mut composite_proof_hashes,
    )
    .unwrap();

    assert_eq!(primitive_proof.outcome, CompatibilityOutcome::Compatible);
    assert_eq!(
        primitive_proof.reason,
        NodeInterfaceSatisfactionReason::Satisfied
    );
    assert_eq!(primitive_proof.members, composite_proof.members);
    assert_ne!(primitive_proof.identity, composite_proof.identity);
}

#[test]
fn directional_refinement_and_extra_ports_are_admitted_without_adapters() {
    let members = [
        member(
            "request",
            Direction::Input,
            InterfaceMemberRequirement::Required,
        ),
        NodeInterfaceMember {
            port: PortContract {
                values: ValueCardinality::ZeroOrMore,
                ..port("response", Direction::Output)
            },
            requirement: InterfaceMemberRequirement::Required,
        },
    ];
    let interface = NodeInterfaceContract {
        id: Id("fixture/request-response"),
        members: &members,
        requirements: &[],
    };
    let interface_ref = interface_reference(&interface);
    let inputs = [
        PortContract {
            values: ValueCardinality::ZeroOrMore,
            connections: ConnectionCardinality::ZeroOrMore,
            ..port("request", Direction::Input)
        },
        port("extra-input", Direction::Input),
    ];
    let outputs = [
        port("response", Direction::Output),
        port("extra-output", Direction::Output),
    ];
    let candidate = NodeContract {
        id: Id("fixture/refined"),
        config: ConfigContract { fields: &[] },
        inputs: &inputs,
        outputs: &outputs,
    };
    let mut member_scratch = vec![placeholder_proof(); 2];
    let mut interface_hashes = [hash(0); 2];
    let mut proof_hashes = [hash(0); 2];
    let proof = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(3),
        &candidate,
        &[],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();

    assert_eq!(proof.outcome, CompatibilityOutcome::Compatible);
    assert!(
        proof
            .members
            .iter()
            .all(|member| member.reason == NodeInterfaceMemberReason::Satisfied)
    );
}

#[test]
fn optionality_only_allows_absence_and_never_hides_an_incompatible_port() {
    let members = [
        member(
            "input",
            Direction::Input,
            InterfaceMemberRequirement::Required,
        ),
        member(
            "partial",
            Direction::Output,
            InterfaceMemberRequirement::Optional,
        ),
    ];
    let interface = NodeInterfaceContract {
        id: Id("fixture/optional-output"),
        members: &members,
        requirements: &[],
    };
    let interface_ref = interface_reference(&interface);
    let inputs = [port("input", Direction::Input)];
    let absent_candidate = NodeContract {
        id: Id("fixture/without-optional"),
        config: ConfigContract { fields: &[] },
        inputs: &inputs,
        outputs: &[],
    };
    let mut member_scratch = vec![placeholder_proof(); 2];
    let mut interface_hashes = [hash(0); 2];
    let mut proof_hashes = [hash(0); 2];
    let absent = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(4),
        &absent_candidate,
        &[],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(absent.outcome, CompatibilityOutcome::Compatible);
    assert_eq!(
        absent.members[1].reason,
        NodeInterfaceMemberReason::OptionalAbsent
    );

    let incompatible_outputs = [PortContract {
        delivery: Delivery::LatestState,
        ..port("partial", Direction::Output)
    }];
    let incompatible_candidate = NodeContract {
        outputs: &incompatible_outputs,
        ..absent_candidate
    };
    let incompatible = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(5),
        &incompatible_candidate,
        &[],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(incompatible.outcome, CompatibilityOutcome::Incompatible);
    assert_eq!(
        incompatible.members[1].port_decision.unwrap().reason,
        PortCompatibilityReason::DeliveryMismatch
    );
}

#[test]
fn missing_wrong_direction_and_complete_port_mismatches_have_stable_reasons() {
    let members = [member(
        "result",
        Direction::Output,
        InterfaceMemberRequirement::Required,
    )];
    let interface = NodeInterfaceContract {
        id: Id("fixture/result-source"),
        members: &members,
        requirements: &[],
    };
    let interface_ref = interface_reference(&interface);
    let empty = NodeContract {
        id: Id("fixture/empty"),
        config: ConfigContract { fields: &[] },
        inputs: &[],
        outputs: &[],
    };
    let mut member_scratch = vec![placeholder_proof()];
    let mut interface_hashes = [hash(0)];
    let mut proof_hashes = [hash(0)];
    let missing = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(6),
        &empty,
        &[],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(
        missing.reason,
        NodeInterfaceSatisfactionReason::MissingRequiredMember
    );

    let wrong_inputs = [port("result", Direction::Input)];
    let wrong = NodeContract {
        inputs: &wrong_inputs,
        ..empty
    };
    let wrong_direction = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(7),
        &wrong,
        &[],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(
        wrong_direction.reason,
        NodeInterfaceSatisfactionReason::WrongDirection
    );

    for incompatible_port in [
        PortContract {
            delivery: Delivery::LatestState,
            ..port("result", Direction::Output)
        },
        PortContract {
            terminal: TerminalContract::OpenEnded,
            ..port("result", Direction::Output)
        },
        PortContract {
            sensitivity: Sensitivity::Secret,
            ..port("result", Direction::Output)
        },
        PortContract {
            flow: PortFlowConstraints {
                loss: LossAcceptance::TypeContractDefined,
            },
            ..port("result", Direction::Output)
        },
    ] {
        let outputs = [incompatible_port];
        let candidate = NodeContract {
            outputs: &outputs,
            ..empty
        };
        let mut local_member_scratch = [placeholder_proof()];
        let mut local_interface_hashes = [hash(0)];
        let mut local_proof_hashes = [hash(0)];
        let proof = assess_node_interface(
            &interface,
            interface_ref,
            candidate_reference(8),
            &candidate,
            &[],
            &[],
            &mut local_member_scratch,
            &mut [],
            &mut local_interface_hashes,
            &mut local_proof_hashes,
        )
        .unwrap();
        assert_eq!(proof.outcome, CompatibilityOutcome::Incompatible);
        assert_eq!(
            proof.reason,
            NodeInterfaceSatisfactionReason::IncompatibleMember
        );
    }
}

#[test]
fn non_exact_types_require_one_reasoned_provider_decision() {
    let members = [member(
        "result",
        Direction::Output,
        InterfaceMemberRequirement::Required,
    )];
    let interface = NodeInterfaceContract {
        id: Id("fixture/provider-output"),
        members: &members,
        requirements: &[],
    };
    let interface_ref = interface_reference(&interface);
    let outputs = [PortContract {
        value_type: OTHER_VALUE,
        ..port("result", Direction::Output)
    }];
    let candidate = NodeContract {
        id: Id("fixture/other-provider"),
        config: ConfigContract { fields: &[] },
        inputs: &[],
        outputs: &outputs,
    };
    let mut member_scratch = vec![placeholder_proof()];
    let mut interface_hashes = [hash(0)];
    let mut proof_hashes = [hash(0)];
    let unavailable = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(9),
        &candidate,
        &[],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(unavailable.outcome, CompatibilityOutcome::Indeterminate);
    assert_eq!(
        unavailable.reason,
        NodeInterfaceSatisfactionReason::ProviderUnavailable
    );

    let provider_decision = NodeInterfaceTypeDecision {
        member_id: Id("result"),
        direction: Direction::Output,
        decision: CompatibilityDecision::compatible(
            CompatibilityQuery::ConsumerAcceptsProducer {
                consumer: VALUE,
                producer: OTHER_VALUE,
            },
            CompatibilityClass::Accepted,
            CompatibilityReason::TypeProviderAccepted,
            None,
        ),
    };
    let accepted = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(9),
        &candidate,
        &[provider_decision],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(accepted.outcome, CompatibilityOutcome::Compatible);
    assert_eq!(
        accepted.members[0].type_decision,
        Some(provider_decision.decision)
    );

    let ambiguous = assess_node_interface(
        &interface,
        interface_ref,
        candidate_reference(9),
        &candidate,
        &[provider_decision, provider_decision],
        &[],
        &mut member_scratch,
        &mut [],
        &mut interface_hashes,
        &mut proof_hashes,
    )
    .unwrap();
    assert_eq!(ambiguous.outcome, CompatibilityOutcome::Indeterminate);
    assert_eq!(ambiguous.reason, NodeInterfaceSatisfactionReason::Ambiguous);
}

#[test]
fn malformed_revisions_duplicates_and_insufficient_scratch_fail_closed() {
    let members = [member(
        "result",
        Direction::Output,
        InterfaceMemberRequirement::Required,
    )];
    let interface = NodeInterfaceContract {
        id: Id("fixture/result-source"),
        members: &members,
        requirements: &[],
    };
    let duplicate_members = [members[0], members[0]];
    assert_eq!(
        NodeInterfaceContract {
            members: &duplicate_members,
            ..interface
        }
        .validate(),
        Err(NodeInterfaceContractError::DuplicateMember {
            id: Id("result"),
            direction: Direction::Output,
        })
    );
    let interface_ref = interface_reference(&interface);
    let outputs = [port("result", Direction::Output)];
    let candidate = NodeContract {
        id: Id("fixture/source"),
        config: ConfigContract { fields: &[] },
        inputs: &[],
        outputs: &outputs,
    };
    let mut member_scratch = vec![placeholder_proof()];
    let mut interface_hashes = [hash(0)];
    let mut proof_hashes = [hash(0)];
    let stale_ref = NodeInterfaceContractRef {
        semantic_hash: hash(99),
        ..interface_ref
    };
    assert_eq!(
        assess_node_interface(
            &interface,
            stale_ref,
            candidate_reference(10),
            &candidate,
            &[],
            &[],
            &mut member_scratch,
            &mut [],
            &mut interface_hashes,
            &mut proof_hashes,
        ),
        Err(NodeInterfaceSatisfactionError::InterfaceIdentity(
            NodeInterfaceIdentityError::ReferenceMismatch
        ))
    );
    assert_eq!(
        assess_node_interface(
            &interface,
            interface_ref,
            candidate_reference(10),
            &candidate,
            &[],
            &[],
            &mut [],
            &mut [],
            &mut interface_hashes,
            &mut proof_hashes,
        ),
        Err(NodeInterfaceSatisfactionError::MemberScratchTooSmall)
    );
    assert_eq!(
        assess_node_interface(
            &interface,
            interface_ref,
            candidate_reference(10),
            &candidate,
            &[],
            &[],
            &mut member_scratch,
            &mut [],
            &mut interface_hashes,
            &mut [],
        ),
        Err(NodeInterfaceSatisfactionError::HashScratchTooSmall)
    );

    let duplicate_outputs = [outputs[0], outputs[0]];
    let malformed_candidate = NodeContract {
        outputs: &duplicate_outputs,
        ..candidate
    };
    assert_eq!(
        assess_node_interface(
            &interface,
            interface_ref,
            candidate_reference(10),
            &malformed_candidate,
            &[],
            &[],
            &mut member_scratch,
            &mut [],
            &mut interface_hashes,
            &mut proof_hashes,
        ),
        Err(NodeInterfaceSatisfactionError::InvalidCandidateContract)
    );
}
