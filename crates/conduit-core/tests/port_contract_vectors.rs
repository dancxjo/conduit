use conduit_core::{
    CompatibilityDecision, CompatibilityOutcome, CompatibilityQuery, CompatibilityReason,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, PortCompatibilityReason,
    PortContract, PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract,
    TerminalContract, TypeContractRef, ValueCardinality, assess_port_connection,
    assess_port_substitution, assess_type_contract_exact,
};

fn parse_direction(value: &str) -> Direction {
    match value {
        "input" => Direction::Input,
        "output" => Direction::Output,
        value => panic!("unknown direction: {value}"),
    }
}

fn parse_presence(value: &str) -> Presence {
    match value {
        "required" => Presence::Required,
        "optional" => Presence::Optional,
        value => panic!("unknown presence: {value}"),
    }
}

fn parse_cardinality(value: &str) -> ConnectionCardinality {
    match value {
        "exactly-one" => ConnectionCardinality::ExactlyOne,
        "zero-or-one" => ConnectionCardinality::ZeroOrOne,
        "one-or-more" => ConnectionCardinality::OneOrMore,
        "zero-or-more" => ConnectionCardinality::ZeroOrMore,
        value => panic!("unknown connection cardinality: {value}"),
    }
}

fn parse_values(value: &str) -> ValueCardinality {
    match value {
        "exactly-one" => ValueCardinality::ExactlyOne,
        "zero-or-one" => ValueCardinality::ZeroOrOne,
        "one-or-more" => ValueCardinality::OneOrMore,
        "zero-or-more" => ValueCardinality::ZeroOrMore,
        value => panic!("unknown value cardinality: {value}"),
    }
}

fn parse_delivery(value: &str) -> Delivery {
    match value {
        "stream" => Delivery::Stream,
        "latest-state" => Delivery::LatestState,
        "finite-batch" => Delivery::FiniteBatch,
        value => panic!("unknown delivery: {value}"),
    }
}

fn parse_temporal(value: &str) -> TemporalContract {
    match value {
        "atemporal" => TemporalContract::Atemporal,
        "progressive" => TemporalContract::Progressive,
        "committed" => TemporalContract::Committed,
        value => panic!("unknown temporal contract: {value}"),
    }
}

fn parse_terminal(value: &str) -> TerminalContract {
    match value {
        "finite" => TerminalContract::Finite,
        "open-ended" => TerminalContract::OpenEnded,
        "either" => TerminalContract::Either,
        value => panic!("unknown terminal contract: {value}"),
    }
}

fn parse_sensitivity(value: &str) -> Sensitivity {
    match value {
        "public" => Sensitivity::Public,
        "restricted" => Sensitivity::Restricted,
        "secret" => Sensitivity::Secret,
        value => panic!("unknown sensitivity: {value}"),
    }
}

fn parse_loss(value: &str) -> LossAcceptance {
    match value {
        "lossless-only" => LossAcceptance::LosslessOnly,
        "type-contract-defined" => LossAcceptance::TypeContractDefined,
        value => panic!("unknown loss acceptance: {value}"),
    }
}

fn type_ref(value: &str) -> TypeContractRef<'static> {
    match value {
        "a" => TypeContractRef {
            contract_id: Id("fixture/type-a"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([
                0x30, 0x59, 0x01, 0xdf, 0x23, 0x30, 0x60, 0x2d, 0x0d, 0x93, 0xbb, 0xa6, 0x75, 0x6e,
                0x70, 0x20, 0x9a, 0x5b, 0xea, 0xe0, 0x5f, 0xed, 0xf6, 0xb9, 0xc5, 0xcf, 0xe5, 0x51,
                0x86, 0x5b, 0xa4, 0xe2,
            ]),
        },
        "b" => TypeContractRef {
            contract_id: Id("fixture/type-b"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([
                0x93, 0xf1, 0xce, 0x8a, 0xb2, 0x59, 0x98, 0x01, 0x1c, 0x18, 0xee, 0x61, 0xd3, 0x01,
                0x1b, 0x90, 0xe7, 0x1a, 0xb3, 0x9f, 0xb8, 0xa7, 0xcc, 0x44, 0xd3, 0x73, 0xc5, 0x4b,
                0x23, 0x76, 0xba, 0xfb,
            ]),
        },
        value => panic!("unknown type fixture: {value}"),
    }
}

fn port<'a>(id: Id<'a>, columns: &[&str], offset: usize) -> PortContract<'a> {
    PortContract {
        id,
        direction: parse_direction(columns[2 + offset]),
        value_type: type_ref(columns[4 + offset]),
        presence: parse_presence(columns[6 + offset]),
        connections: parse_cardinality(columns[8 + offset]),
        values: parse_values(columns[10 + offset]),
        delivery: parse_delivery(columns[12 + offset]),
        temporal: parse_temporal(columns[14 + offset]),
        terminal: parse_terminal(columns[16 + offset]),
        sensitivity: parse_sensitivity(columns[18 + offset]),
        flow: PortFlowConstraints {
            loss: parse_loss(columns[20 + offset]),
        },
    }
}

fn type_decision<'a>(
    consumer: TypeContractRef<'a>,
    producer: TypeContractRef<'a>,
) -> CompatibilityDecision<'a> {
    let exact = assess_type_contract_exact(consumer, producer);
    if exact.outcome == CompatibilityOutcome::Compatible {
        exact
    } else {
        CompatibilityDecision::incompatible(
            CompatibilityQuery::ConsumerAcceptsProducer { consumer, producer },
            CompatibilityReason::TypeProviderRejected,
            None,
        )
    }
}

fn parse_outcome(value: &str) -> CompatibilityOutcome {
    match value {
        "compatible" => CompatibilityOutcome::Compatible,
        "incompatible" => CompatibilityOutcome::Incompatible,
        value => panic!("unknown outcome: {value}"),
    }
}

#[test]
fn port_compatibility_matches_frozen_vectors() {
    let fixtures = include_str!("../../../conformance/c2/port-contract.tsv");
    for line in fixtures.lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 25, "invalid fixture row: {line}");
        let consumer = port(Id("consumer"), &columns, 0);
        let producer = port(Id("producer"), &columns, 1);
        let types = type_decision(consumer.value_type, producer.value_type);
        let decision = match columns[1] {
            "connection" => assess_port_connection(consumer, producer, types),
            "substitution" => assess_port_substitution(consumer, producer, types),
            value => panic!("unknown fixture mode: {value}"),
        };

        assert_eq!(
            decision.outcome,
            parse_outcome(columns[22]),
            "{} outcome",
            columns[0]
        );
        assert_eq!(
            decision.reason.as_str(),
            columns[23],
            "{} reason",
            columns[0]
        );
        assert_eq!(decision.consumer, consumer, "{} consumer", columns[0]);
        assert_eq!(decision.producer, producer, "{} producer", columns[0]);
        assert_eq!(
            consumer.direction.as_str(),
            columns[2],
            "{} direction round trip",
            columns[0]
        );
        assert_eq!(
            consumer.connections.as_str(),
            columns[8],
            "{} connections round trip",
            columns[0]
        );
        assert_eq!(
            consumer.values.as_str(),
            columns[10],
            "{} values round trip",
            columns[0]
        );
        assert_eq!(
            consumer.delivery.as_str(),
            columns[12],
            "{} delivery round trip",
            columns[0]
        );
        assert_eq!(
            consumer.temporal.as_str(),
            columns[14],
            "{} temporal round trip",
            columns[0]
        );
        assert_eq!(
            consumer.terminal.as_str(),
            columns[16],
            "{} terminal round trip",
            columns[0]
        );
        assert_eq!(
            consumer.sensitivity.as_str(),
            columns[18],
            "{} sensitivity round trip",
            columns[0]
        );
        assert_eq!(
            consumer.flow.loss.as_str(),
            columns[20],
            "{} loss round trip",
            columns[0]
        );

        if columns[24] != "-" {
            let hash = consumer.semantic_hash().unwrap().to_string();
            assert_eq!(hash, columns[24], "{} descriptor hash", columns[0]);
        }
    }
}

#[test]
fn every_port_reason_has_a_stable_spelling() {
    let reasons = [
        PortCompatibilityReason::Accepted,
        PortCompatibilityReason::DirectionMismatch,
        PortCompatibilityReason::TypeMismatch,
        PortCompatibilityReason::PresenceMismatch,
        PortCompatibilityReason::ConnectionCardinalityMismatch,
        PortCompatibilityReason::ValueCardinalityMismatch,
        PortCompatibilityReason::DeliveryMismatch,
        PortCompatibilityReason::TemporalMismatch,
        PortCompatibilityReason::TerminalMismatch,
        PortCompatibilityReason::SensitivityViolation,
        PortCompatibilityReason::FlowConstraintMismatch,
    ];
    for reason in reasons {
        assert!(reason.as_str().starts_with("port-"));
    }
}

#[test]
fn a_type_decision_for_different_operands_is_not_reused() {
    let fixtures = include_str!("../../../conformance/c2/port-contract.tsv");
    let columns = fixtures
        .lines()
        .find(|line| line.starts_with("accepted\t"))
        .unwrap()
        .split('\t')
        .collect::<Vec<_>>();
    let consumer = port(Id("consumer"), &columns, 0);
    let producer = port(Id("producer"), &columns, 1);
    let unrelated = type_ref("b");
    let wrong = CompatibilityDecision::compatible(
        CompatibilityQuery::ConsumerAcceptsProducer {
            consumer: unrelated,
            producer: unrelated,
        },
        conduit_core::CompatibilityClass::Exact,
        CompatibilityReason::TypeContractExact,
        None,
    );

    let decision = assess_port_connection(consumer, producer, wrong);
    assert_eq!(decision.outcome, CompatibilityOutcome::Indeterminate);
    assert_eq!(decision.reason, PortCompatibilityReason::TypeMismatch);
}

#[test]
fn every_port_contract_fact_changes_identity() {
    let fixtures = include_str!("../../../conformance/c2/port-contract.tsv");
    let columns = fixtures
        .lines()
        .find(|line| line.starts_with("accepted\t"))
        .unwrap()
        .split('\t')
        .collect::<Vec<_>>();
    let base = port(Id("consumer"), &columns, 0);
    let base_hash = base.semantic_hash().unwrap();
    let variants = [
        PortContract {
            id: Id("other"),
            ..base
        },
        PortContract {
            direction: Direction::Output,
            ..base
        },
        PortContract {
            value_type: type_ref("b"),
            ..base
        },
        PortContract {
            presence: Presence::Optional,
            ..base
        },
        PortContract {
            connections: ConnectionCardinality::ZeroOrMore,
            ..base
        },
        PortContract {
            values: ValueCardinality::ZeroOrMore,
            ..base
        },
        PortContract {
            delivery: Delivery::Stream,
            ..base
        },
        PortContract {
            temporal: TemporalContract::Committed,
            ..base
        },
        PortContract {
            terminal: TerminalContract::OpenEnded,
            ..base
        },
        PortContract {
            sensitivity: Sensitivity::Restricted,
            ..base
        },
        PortContract {
            flow: PortFlowConstraints {
                loss: LossAcceptance::TypeContractDefined,
            },
            ..base
        },
    ];

    for variant in variants {
        assert_ne!(base_hash, variant.semantic_hash().unwrap());
    }
}
