use conduit_core::{
    CompatibilityClass, CompatibilityOutcome, CompatibilityReason, DescriptorRef, Id, MigrationRef,
    RecordField, RecordSchema, SemanticHash, UnknownFieldPolicy, ValueAcceptance, assess_exact,
    assess_migration, assess_reader_acceptance,
};

const ACCEPT_11: [SemanticHash; 1] = [SemanticHash::from_bytes([0x11; 32])];

fn hash(seed: &str) -> SemanticHash {
    let seed = u8::from_str_radix(seed, 16).expect("fixture hash seed");
    SemanticHash::from_bytes([seed; 32])
}

fn parse_fields(source: &'static str) -> Vec<RecordField<'static>> {
    if source == "-" {
        return Vec::new();
    }
    source
        .split(';')
        .map(|field| {
            let parts = field.split(',').collect::<Vec<_>>();
            assert_eq!(parts.len(), 5, "invalid field fixture: {field}");
            let accepts = match parts[4] {
                "exact" => ValueAcceptance::Exact,
                "provider" => ValueAcceptance::ProviderRequired,
                "oneof11" => ValueAcceptance::ExactOr(&ACCEPT_11),
                value => panic!("unknown acceptance fixture: {value}"),
            };
            RecordField {
                id: Id(parts[0]),
                required: match parts[1] {
                    "r" => true,
                    "o" => false,
                    value => panic!("unknown presence fixture: {value}"),
                },
                value_contract: hash(parts[2]),
                accepts,
                default: (parts[3] != "-").then(|| hash(parts[3])),
            }
        })
        .collect()
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
        "backward-compatible" => Some(CompatibilityClass::BackwardCompatible),
        "forward-compatible" => Some(CompatibilityClass::ForwardCompatible),
        "migratable" => Some(CompatibilityClass::Migratable),
        value => panic!("unknown class fixture: {value}"),
    }
}

fn parse_reason(value: &str) -> CompatibilityReason {
    match value {
        "exact-identity" => CompatibilityReason::ExactIdentity,
        "fields-accepted" => CompatibilityReason::FieldsAccepted,
        "descriptor-kind-mismatch" => CompatibilityReason::DescriptorKindMismatch,
        "missing-required-field" => CompatibilityReason::MissingRequiredField,
        "unknown-producer-field" => CompatibilityReason::UnknownProducerField,
        "value-contract-rejected" => CompatibilityReason::ValueContractRejected,
        "value-provider-required" => CompatibilityReason::ValueProviderRequired,
        "default-changed" => CompatibilityReason::DefaultChanged,
        "migration-source-mismatch" => CompatibilityReason::MigrationSourceMismatch,
        "migration-target-mismatch" => CompatibilityReason::MigrationTargetMismatch,
        "migration-not-deterministic" => CompatibilityReason::MigrationNotDeterministic,
        "migration-not-total" => CompatibilityReason::MigrationNotTotal,
        "migration-accepted" => CompatibilityReason::MigrationAccepted,
        value => panic!("unknown reason fixture: {value}"),
    }
}

#[test]
fn record_compatibility_matches_frozen_vectors() {
    let fixtures = include_str!("../../../conformance/c1/compatibility.tsv");
    for line in fixtures.lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 14, "invalid fixture row: {line}");
        let reader_fields = parse_fields(columns[8]);
        let writer_fields = parse_fields(columns[9]);
        let reader = RecordSchema {
            descriptor: DescriptorRef {
                kind: Id(columns[1]),
                schema_version: columns[3].parse().unwrap(),
                semantic_hash: hash(columns[5]),
            },
            fields: &reader_fields,
            unknown_fields: match columns[7] {
                "reject" => UnknownFieldPolicy::Reject,
                "preserve" => UnknownFieldPolicy::Preserve,
                value => panic!("unknown unknown-field fixture: {value}"),
            },
        };
        let writer = RecordSchema {
            descriptor: DescriptorRef {
                kind: Id(columns[2]),
                schema_version: columns[4].parse().unwrap(),
                semantic_hash: hash(columns[6]),
            },
            fields: &writer_fields,
            unknown_fields: UnknownFieldPolicy::Reject,
        };

        let decision = assess_reader_acceptance(&reader, &writer);
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
fn migration_compatibility_matches_frozen_vectors() {
    let fixtures = include_str!("../../../conformance/c1/migration.tsv");
    for line in fixtures.lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 12, "invalid fixture row: {line}");
        let source = DescriptorRef {
            kind: Id("conduit/schema"),
            schema_version: columns[1].parse().unwrap(),
            semantic_hash: hash(columns[3]),
        };
        let target = DescriptorRef {
            kind: Id("conduit/schema"),
            schema_version: columns[2].parse().unwrap(),
            semantic_hash: hash(columns[4]),
        };
        let migration = MigrationRef {
            id: Id("conduit/schema-to-target"),
            semantic_hash: hash("99"),
            source: DescriptorRef {
                semantic_hash: hash(columns[5]),
                ..source
            },
            target: DescriptorRef {
                semantic_hash: hash(columns[6]),
                ..target
            },
            deterministic: columns[7].parse().unwrap(),
            total: columns[8].parse().unwrap(),
        };

        let decision = assess_migration(source, target, migration);
        assert_eq!(
            decision.outcome,
            parse_outcome(columns[9]),
            "{} outcome",
            columns[0]
        );
        assert_eq!(
            decision.class,
            parse_class(columns[10]),
            "{} class",
            columns[0]
        );
        assert_eq!(
            decision.reason,
            parse_reason(columns[11]),
            "{} reason",
            columns[0]
        );
        assert_eq!(
            decision.migration,
            (decision.outcome == CompatibilityOutcome::Compatible)
                .then_some(migration.semantic_hash),
            "{} migration identity",
            columns[0]
        );
    }
}

#[test]
fn exact_compatibility_is_not_version_or_hash_similarity() {
    let exact = DescriptorRef {
        kind: Id("conduit/schema"),
        schema_version: 0,
        semantic_hash: hash("11"),
    };
    assert_eq!(
        assess_exact(exact, exact).class,
        Some(CompatibilityClass::Exact)
    );
    assert_eq!(
        assess_exact(
            exact,
            DescriptorRef {
                schema_version: 99,
                ..exact
            }
        )
        .reason,
        CompatibilityReason::SchemaVersionMismatch
    );
    assert_eq!(
        assess_exact(
            exact,
            DescriptorRef {
                semantic_hash: hash("12"),
                ..exact
            }
        )
        .reason,
        CompatibilityReason::SemanticHashMismatch
    );
    let invalid = DescriptorRef {
        kind: Id("Invalid"),
        ..exact
    };
    assert_eq!(
        assess_exact(invalid, invalid).outcome,
        CompatibilityOutcome::Indeterminate
    );
}

#[test]
fn malformed_schemas_are_indeterminate_not_incompatible() {
    let fields = [
        RecordField {
            id: Id("value"),
            required: true,
            value_contract: hash("11"),
            accepts: ValueAcceptance::Exact,
            default: None,
        },
        RecordField {
            id: Id("value"),
            required: false,
            value_contract: hash("11"),
            accepts: ValueAcceptance::Exact,
            default: None,
        },
    ];
    let descriptor = DescriptorRef {
        kind: Id("conduit/schema"),
        schema_version: 0,
        semantic_hash: hash("11"),
    };
    let malformed = RecordSchema {
        descriptor,
        fields: &fields,
        unknown_fields: UnknownFieldPolicy::Reject,
    };
    let valid = RecordSchema {
        descriptor,
        fields: &fields[..1],
        unknown_fields: UnknownFieldPolicy::Reject,
    };

    let decision = assess_reader_acceptance(&malformed, &valid);
    assert_eq!(decision.outcome, CompatibilityOutcome::Indeterminate);
    assert_eq!(decision.reason, CompatibilityReason::InvalidSchema);
    assert_eq!(decision.subject, Some(Id("value")));
}
