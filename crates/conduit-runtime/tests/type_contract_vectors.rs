use conduit_core::{
    CanonicalDescriptor, CanonicalValue, CompatibilityClass, CompatibilityOutcome,
    CompatibilityQuery, CompatibilityReason, DescriptorRef, FlowTypeFacts, Id, SemanticHash,
    TraitProof, TypeContractRef,
};
use conduit_runtime::{
    ProviderTypeDecision, TypeComparisonStrategy, TypeContractDescription, TypeContractProvider,
    TypeRegistry,
};

struct FixtureProvider;

impl TypeContractProvider for FixtureProvider {
    fn provider_descriptor(&self) -> DescriptorRef<'static> {
        DescriptorRef {
            kind: Id("fixture/type-provider"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([0x91; 32]),
        }
    }

    fn namespace(&self) -> &str {
        "fixture"
    }

    fn describe<'a>(
        &'a self,
        reference: TypeContractRef<'a>,
    ) -> Option<TypeContractDescription<'a>> {
        let strategy = match reference.contract_id.as_str() {
            "fixture/nominal" => TypeComparisonStrategy::Nominal,
            "fixture/record-a" | "fixture/record-b" => TypeComparisonStrategy::Structural {
                shape: SemanticHash::from_bytes([0x31; 32]),
            },
            "fixture/milliseconds" => TypeComparisonStrategy::Structural {
                shape: SemanticHash::from_bytes([0x41; 32]),
            },
            "fixture/samples" => TypeComparisonStrategy::Structural {
                shape: SemanticHash::from_bytes([0x42; 32]),
            },
            "fixture/audio.pcm" | "fixture/audio.stream" => TypeComparisonStrategy::Opaque,
            "fixture/future" if reference.schema_version == 2 => {
                TypeComparisonStrategy::Unknown(Id("fixture/future-strategy"))
            }
            "fixture/future" => TypeComparisonStrategy::Nominal,
            _ => return None,
        };
        Some(TypeContractDescription {
            human_name: reference.contract_id.as_str(),
            descriptor: descriptor(reference.contract_id, reference.schema_version),
            strategy,
            flow_type_facts: FlowTypeFacts {
                disposable: TraitProof::Indeterminate,
                coalescers: None,
            },
        })
    }

    fn consumer_accepts_producer<'a>(
        &'a self,
        consumer: TypeContractRef<'a>,
        producer: TypeContractRef<'a>,
    ) -> ProviderTypeDecision<'a> {
        if consumer.contract_id == Id("fixture/nominal")
            && producer.contract_id == Id("fixture/nominal")
            && consumer.schema_version == 2
            && producer.schema_version == 1
        {
            return ProviderTypeDecision {
                outcome: CompatibilityOutcome::Compatible,
                rule: Id("fixture/accepts"),
            };
        }
        if consumer.contract_id == Id("fixture/audio.pcm")
            && producer.contract_id == Id("fixture/audio.stream")
        {
            return ProviderTypeDecision {
                outcome: CompatibilityOutcome::Incompatible,
                rule: Id("fixture/explicit-adapter-required"),
            };
        }
        ProviderTypeDecision {
            outcome: CompatibilityOutcome::Incompatible,
            rule: Id("fixture/no-provider-rule"),
        }
    }
}

fn descriptor(id: Id<'_>, schema_version: u32) -> CanonicalDescriptor<'_> {
    CanonicalDescriptor {
        kind: id,
        schema_version,
        body: CanonicalValue::Null,
    }
}

fn reference(id: &'static str, schema_version: u32) -> TypeContractRef<'static> {
    let descriptor = descriptor(Id(id), schema_version);
    TypeContractRef {
        contract_id: Id(id),
        schema_version,
        semantic_hash: descriptor.semantic_hash().expect("fixture descriptor"),
    }
}

fn parse_strategy(
    id: &'static str,
    schema_version: u32,
    strategy: &str,
    shape: &str,
) -> TypeContractRef<'static> {
    let reference = reference(id, schema_version);
    let expected = match strategy {
        "nominal" | "opaque" | "unknown" => "-",
        "structural" => shape,
        value => panic!("unknown strategy fixture: {value}"),
    };
    assert_eq!(
        shape, expected,
        "strategy/shape declaration is not canonical"
    );
    reference
}

fn parse_outcome(value: &str) -> CompatibilityOutcome {
    match value {
        "compatible" => CompatibilityOutcome::Compatible,
        "incompatible" => CompatibilityOutcome::Incompatible,
        "indeterminate" => CompatibilityOutcome::Indeterminate,
        value => panic!("unknown outcome fixture: {value}"),
    }
}

fn parse_class(value: &str) -> Option<CompatibilityClass> {
    match value {
        "-" => None,
        "exact" => Some(CompatibilityClass::Exact),
        "accepted" => Some(CompatibilityClass::Accepted),
        value => panic!("unknown class fixture: {value}"),
    }
}

fn parse_reason(value: &str) -> CompatibilityReason {
    match value {
        "type-contract-exact" => CompatibilityReason::TypeContractExact,
        "type-provider-accepted" => CompatibilityReason::TypeProviderAccepted,
        "type-structural-accepted" => CompatibilityReason::TypeStructuralAccepted,
        "type-structural-mismatch" => CompatibilityReason::TypeStructuralMismatch,
        "type-provider-rejected" => CompatibilityReason::TypeProviderRejected,
        "type-provider-unavailable" => CompatibilityReason::TypeProviderUnavailable,
        "type-strategy-unknown" => CompatibilityReason::TypeStrategyUnknown,
        "invalid-type-reference" => CompatibilityReason::InvalidTypeReference,
        value => panic!("unknown reason fixture: {value}"),
    }
}

#[test]
fn type_compatibility_matches_frozen_vectors() {
    let mut registry = TypeRegistry::default();
    registry.register(FixtureProvider).unwrap();

    let fixtures = include_str!("../../../conformance/c2/type-contract.tsv");
    for line in fixtures.lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 14, "invalid fixture row: {line}");
        let consumer = parse_strategy(
            columns[1],
            columns[2].parse().unwrap(),
            columns[5],
            columns[6],
        );
        let producer = parse_strategy(
            columns[3],
            columns[4].parse().unwrap(),
            columns[7],
            columns[8],
        );

        let decision = registry.consumer_accepts_producer(consumer, producer);
        if columns[0] == "same_shape_different_semantics" {
            assert_eq!(
                columns[9], "record-value-u32",
                "wire shape must stay equal while semantic projections differ"
            );
            assert_ne!(columns[6], columns[8], "semantic projections must differ");
        }
        assert_eq!(
            decision.query,
            CompatibilityQuery::ConsumerAcceptsProducer { consumer, producer },
            "{} operands",
            columns[0]
        );
        assert_eq!(
            decision.outcome,
            parse_outcome(columns[10]),
            "{} outcome",
            columns[0]
        );
        assert_eq!(
            decision.class,
            parse_class(columns[11]),
            "{} class",
            columns[0]
        );
        assert_eq!(
            decision.reason,
            parse_reason(columns[12]),
            "{} reason",
            columns[0]
        );
        assert_eq!(
            decision.subject.map(Id::as_str),
            (columns[13] != "-").then_some(columns[13]),
            "{} subject",
            columns[0]
        );
    }
}

#[test]
fn satisfaction_report_retains_provider_rule_and_bilateral_facets() {
    let mut registry = TypeRegistry::default();
    registry.register(FixtureProvider).unwrap();
    let consumer = reference("fixture/record-a", 1);
    let producer = reference("fixture/record-b", 1);

    let report = registry.consumer_satisfaction_report(consumer, producer);

    assert_eq!(report.decision.outcome, CompatibilityOutcome::Compatible);
    assert_eq!(
        report.provider_rule,
        Some(Id("conduit/structural-projection"))
    );
    assert_eq!(
        report.consumer_provider,
        Some(FixtureProvider.provider_descriptor())
    );
    assert_eq!(report.consumer_provider, report.producer_provider);
    assert_eq!(
        report.consumer_structural_facet,
        report.producer_structural_facet
    );
    assert!(report.consumer_structural_facet.is_some());

    let exact = registry.consumer_satisfaction_report(consumer, consumer);
    assert_eq!(exact.decision.class, Some(CompatibilityClass::Exact));
    assert_eq!(exact.consumer_provider, None);
    assert_eq!(exact.producer_provider, None);
    assert_eq!(exact.provider_rule, None);
}

#[test]
fn discovery_returns_the_exact_immutable_descriptor() {
    let mut registry = TypeRegistry::default();
    registry.register(FixtureProvider).unwrap();
    let reference = reference("fixture/audio.pcm", 1);

    let description = registry.describe(reference).expect("known descriptor");
    assert_eq!(description.human_name, "fixture/audio.pcm");
    assert_eq!(
        description.descriptor.semantic_hash().unwrap(),
        reference.semantic_hash
    );
    assert_eq!(description.strategy, TypeComparisonStrategy::Opaque);
}

#[test]
fn registration_rejects_invalid_and_duplicate_namespaces() {
    struct NamedProvider(&'static str);
    impl TypeContractProvider for NamedProvider {
        fn provider_descriptor(&self) -> DescriptorRef<'static> {
            DescriptorRef {
                kind: Id("fixture/named-provider"),
                schema_version: 0,
                semantic_hash: SemanticHash::from_bytes([0x92; 32]),
            }
        }

        fn namespace(&self) -> &str {
            self.0
        }

        fn describe<'a>(&'a self, _: TypeContractRef<'a>) -> Option<TypeContractDescription<'a>> {
            None
        }

        fn consumer_accepts_producer<'a>(
            &'a self,
            _: TypeContractRef<'a>,
            _: TypeContractRef<'a>,
        ) -> ProviderTypeDecision<'a> {
            ProviderTypeDecision {
                outcome: CompatibilityOutcome::Indeterminate,
                rule: Id("fixture/unavailable"),
            }
        }
    }

    let mut registry = TypeRegistry::default();
    assert!(registry.register(NamedProvider("Invalid")).is_err());
    registry.register(NamedProvider("fixture")).unwrap();
    assert!(registry.register(NamedProvider("fixture")).is_err());
}
