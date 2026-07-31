use conduit_core::{
    CanonicalValue, ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability,
    ConfigRequirement, Id, SemanticHash, Sensitivity, TypeContractRef,
};
use conduit_runtime::{
    ConfigAssignment, ConfigResolutionError, ConfigValue, SecretValue, TypeRegistry,
    resolve_config, validate_config_update,
};

const TEXT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xc1, 0xbb, 0x91, 0xcf, 0x01, 0xee, 0xfc, 0xeb, 0x30, 0x4e, 0xe1, 0xfb, 0x4d, 0xe5, 0xb8,
        0x7f, 0xee, 0x8e, 0xa2, 0x74, 0xb0, 0x9b, 0xc9, 0x6f, 0x72, 0xe2, 0xcc, 0xa8, 0x95, 0x75,
        0x03, 0x08,
    ]),
};
const DEFAULT: CanonicalValue<'static> = CanonicalValue::Text("neutral");
const DEFAULT_FIELD: ConfigFieldContract<'static> = ConfigFieldContract {
    key: Id("tone"),
    value_type: TEXT,
    requirement: ConfigRequirement::Defaulted(DEFAULT),
    sensitivity: Sensitivity::Public,
    mutability: ConfigMutability::PreStart,
    identity: ConfigIdentity::Semantic,
};
const SECRET_FIELD: ConfigFieldContract<'static> = ConfigFieldContract {
    key: Id("credential"),
    value_type: TEXT,
    requirement: ConfigRequirement::Required,
    sensitivity: Sensitivity::Secret,
    mutability: ConfigMutability::Runtime,
    identity: ConfigIdentity::Plan,
};

fn public_assignment(key: &'static str, value: &'static str) -> ConfigAssignment<'static> {
    ConfigAssignment {
        key: Id(key),
        value_type: TEXT,
        value: ConfigValue::Public(CanonicalValue::Text(value)),
    }
}

fn secret_assignment(key: &'static str, value: &'static str) -> ConfigAssignment<'static> {
    ConfigAssignment {
        key: Id(key),
        value_type: TEXT,
        value: ConfigValue::Secret(SecretValue::new(value)),
    }
}

fn captured<T>(result: Result<T, ConfigResolutionError<'_>>) -> Result<T, (&'static str, String)> {
    result.map_err(|error| (error.code(), error.to_string()))
}

#[test]
fn config_fixtures_preserve_defaults_mutability_and_redaction() {
    let registry = TypeRegistry::default();
    let fixtures = include_str!("../../../conformance/c2/config.tsv");
    let default_contract = ConfigContract {
        fields: &[DEFAULT_FIELD],
    };
    let secret_contract = ConfigContract {
        fields: &[SECRET_FIELD],
    };
    let public_required = ConfigFieldContract {
        sensitivity: Sensitivity::Public,
        ..SECRET_FIELD
    };
    let public_contract = ConfigContract {
        fields: &[public_required],
    };
    let mut implicit_hash = None;

    for line in fixtures.lines().filter(|line| !line.starts_with('#')) {
        let columns = line.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 8, "invalid fixture row: {line}");
        if columns[6] != "-" {
            assert_eq!(
                DEFAULT_FIELD.semantic_hash().unwrap().to_string(),
                columns[6],
                "{} field descriptor hash",
                columns[0]
            );
        }
        if columns[7] != "-" {
            assert_eq!(
                default_contract.semantic_hash().unwrap().to_string(),
                columns[7],
                "{} contract descriptor hash",
                columns[0]
            );
        }
        let result = match columns[0] {
            "canonical_default" => {
                captured(resolve_config(&registry, default_contract, &[])).map(|resolved| {
                    let entry = resolved.get("tone").unwrap();
                    assert!(entry.defaulted);
                    implicit_hash = Some(resolved.semantic_hash().unwrap());
                })
            }
            "explicit_default" => {
                let assignments = [public_assignment("tone", columns[2])];
                captured(resolve_config(&registry, default_contract, &assignments)).map(
                    |resolved| {
                        assert!(!resolved.get("tone").unwrap().defaulted);
                        assert_eq!(
                            Some(resolved.semantic_hash().unwrap()),
                            implicit_hash,
                            "explicit and implicit canonical defaults must agree"
                        );
                    },
                )
            }
            "secret_value" => {
                let assignments = [secret_assignment("credential", columns[2])];
                assert!(!format!("{assignments:?}").contains(columns[2]));
                captured(resolve_config(&registry, secret_contract, &assignments)).map(|resolved| {
                    assert!(!format!("{resolved:?}").contains(columns[2]));
                    assert_eq!(
                        resolved.get("credential").unwrap().value,
                        ConfigValue::Secret(SecretValue::new(columns[2]))
                    );
                })
            }
            "pre_start_mutation" => {
                let assignment = public_assignment("tone", columns[2]);
                captured(validate_config_update(
                    &registry,
                    default_contract,
                    &assignment,
                ))
            }
            "secret_to_public" => {
                let assignments = [secret_assignment("credential", columns[2])];
                captured(resolve_config(&registry, public_contract, &assignments)).map(|_| ())
            }
            value => panic!("unknown config fixture: {value}"),
        };

        match columns[4] {
            "compatible" => assert!(result.is_ok(), "{}: {result:?}", columns[0]),
            "incompatible" => {
                let (code, message) = result.expect_err(columns[0]);
                assert_eq!(code, columns[5], "{} code", columns[0]);
                assert!(
                    !message.contains(columns[2]),
                    "{} leaked supplied value",
                    columns[0]
                );
            }
            value => panic!("unknown outcome: {value}"),
        }
    }
}

#[test]
fn configuration_fields_are_not_patchable_ports() {
    let contract = conduit_core::NodeContract {
        id: Id("fixture/configured"),
        config: ConfigContract {
            fields: &[DEFAULT_FIELD],
        },
        inputs: &[],
        outputs: &[],
    };

    assert_eq!(contract.config.fields[0].key, Id("tone"));
    assert!(contract.inputs.is_empty());
    assert!(contract.outputs.is_empty());
}

#[test]
fn field_descriptor_hash_changes_with_default_semantics() {
    let first = DEFAULT_FIELD.semantic_hash().unwrap();
    let changed = ConfigFieldContract {
        requirement: ConfigRequirement::Defaulted(CanonicalValue::Text("warm")),
        ..DEFAULT_FIELD
    };

    assert_ne!(first, changed.semantic_hash().unwrap());
}

#[test]
fn every_config_field_contract_fact_changes_identity() {
    let base = DEFAULT_FIELD.semantic_hash().unwrap();
    let variants = [
        ConfigFieldContract {
            key: Id("style"),
            ..DEFAULT_FIELD
        },
        ConfigFieldContract {
            value_type: TypeContractRef {
                semantic_hash: SemanticHash::from_bytes([0xaa; 32]),
                ..TEXT
            },
            ..DEFAULT_FIELD
        },
        ConfigFieldContract {
            requirement: ConfigRequirement::Optional,
            ..DEFAULT_FIELD
        },
        ConfigFieldContract {
            sensitivity: Sensitivity::Restricted,
            identity: ConfigIdentity::Plan,
            ..DEFAULT_FIELD
        },
        ConfigFieldContract {
            mutability: ConfigMutability::Runtime,
            ..DEFAULT_FIELD
        },
        ConfigFieldContract {
            identity: ConfigIdentity::Plan,
            ..DEFAULT_FIELD
        },
    ];

    for variant in variants {
        assert_ne!(base, variant.semantic_hash().unwrap());
    }
}

#[test]
fn config_contract_identity_ignores_field_order() {
    let second = ConfigFieldContract {
        key: Id("volume"),
        requirement: ConfigRequirement::Optional,
        ..DEFAULT_FIELD
    };
    let first_order = [DEFAULT_FIELD, second];
    let second_order = [second, DEFAULT_FIELD];

    assert_eq!(
        ConfigContract {
            fields: &first_order
        }
        .semantic_hash()
        .unwrap(),
        ConfigContract {
            fields: &second_order
        }
        .semantic_hash()
        .unwrap()
    );
}
