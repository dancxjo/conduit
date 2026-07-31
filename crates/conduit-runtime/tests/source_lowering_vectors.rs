use std::collections::BTreeMap;

use conduit_core::{
    CanonicalValue, ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability,
    ConfigRequirement, Id, NodeContract, SemanticHash, Sensitivity, TypeContractRef,
};
use conduit_panel::{
    LoadedModule, ModuleLoader, RootSelectionMode, SourceValue, resolve_modules,
    semantic_source_hash,
};
use conduit_runtime::{
    ConfigProvenance, LiteralValidationError, LoweredConfigValue, OwnedConfigFieldSchema,
    OwnedConfigRequirement, OwnedNodeSchema, OwnedPortReference, OwnedSemanticValue,
    OwnedTypeReference, SourceContractCatalog, lower_source, lower_source_base, lower_topology,
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

struct MemoryLoader(BTreeMap<String, String>);

impl ModuleLoader for MemoryLoader {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
            canonical_uri: canonical_uri.to_owned(),
            source: source.clone(),
        }))
    }
}

#[derive(Default)]
struct Catalog;

impl Catalog {
    fn type_ref(id: &str) -> OwnedTypeReference {
        OwnedTypeReference {
            id: id.to_owned(),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes(Sha256::digest(id.as_bytes()).into()),
        }
    }

    fn field(key: &str, kind: &str, requirement: OwnedConfigRequirement) -> OwnedConfigFieldSchema {
        OwnedConfigFieldSchema {
            key: key.to_owned(),
            value_type: Self::type_ref(kind),
            requirement,
            sensitivity: Sensitivity::Public,
            mutability: ConfigMutability::PreStart,
            identity: ConfigIdentity::Semantic,
            default_origin: None,
        }
    }
}

impl SourceContractCatalog for Catalog {
    fn node_schema(&self, id: &str) -> Option<OwnedNodeSchema> {
        let fields = match id {
            "fixture/defaulted" => vec![Self::field(
                "enabled",
                "fixture/bool",
                OwnedConfigRequirement::Defaulted(OwnedSemanticValue::Boolean(true)),
            )],
            "fixture/integer" => vec![Self::field(
                "count",
                "fixture/i8",
                OwnedConfigRequirement::Required,
            )],
            "fixture/record" => vec![Self::field(
                "value",
                "fixture/record",
                OwnedConfigRequirement::Required,
            )],
            "fixture/decimal" => vec![Self::field(
                "value",
                "fixture/decimal",
                OwnedConfigRequirement::Required,
            )],
            "fixture/provider" => vec![Self::field(
                "value",
                "fixture/unavailable",
                OwnedConfigRequirement::Required,
            )],
            "fixture/secret" => {
                let mut field = Self::field(
                    "token",
                    "fixture/secret-ref",
                    OwnedConfigRequirement::Required,
                );
                field.sensitivity = Sensitivity::Secret;
                field.identity = ConfigIdentity::Plan;
                vec![field]
            }
            "fixture/all-literals" => vec![
                Self::field("flag", "fixture/bool", OwnedConfigRequirement::Required),
                Self::field("count", "fixture/i8", OwnedConfigRequirement::Required),
                Self::field("text", "fixture/text", OwnedConfigRequirement::Required),
                Self::field("data", "fixture/bytes", OwnedConfigRequirement::Required),
                Self::field("target", "fixture/ref", OwnedConfigRequirement::Required),
                Self::field(
                    "schema",
                    "fixture/contract-ref",
                    OwnedConfigRequirement::Required,
                ),
                Self::field(
                    "items",
                    "fixture/list-bool",
                    OwnedConfigRequirement::Required,
                ),
                Self::field("nested", "fixture/record", OwnedConfigRequirement::Required),
                Self::field(
                    "amount",
                    "fixture/decimal",
                    OwnedConfigRequirement::Required,
                ),
            ],
            "fixture/handler"
            | "fixture/speech-node"
            | "fixture/incomplete-node"
            | "fixture/swapped-directions" => Vec::new(),
            _ => return None,
        };
        Some(OwnedNodeSchema {
            id: id.to_owned(),
            fields,
        })
    }

    fn type_reference(&self, id: &str) -> Option<OwnedTypeReference> {
        id.starts_with("fixture/").then(|| Self::type_ref(id))
    }

    fn port_contract(&self, id: &str) -> Option<OwnedPortReference> {
        let direction = match id {
            "fixture/request-port" => conduit_core::Direction::Input,
            "fixture/reply-port" => conduit_core::Direction::Output,
            "fixture/wrong-direction" => conduit_core::Direction::Output,
            _ => return None,
        };
        Some(OwnedPortReference {
            id: id.to_owned(),
            direction,
            semantic_hash: Self::type_ref(id).semantic_hash,
        })
    }

    fn node_contract(&self, id: &str) -> Option<conduit_runtime::OwnedNodeContract> {
        fn port(
            id: &str,
            dir: conduit_core::Direction,
            type_id: &str,
        ) -> conduit_runtime::OwnedPortContract {
            conduit_runtime::OwnedPortContract {
                id: id.to_owned(),
                direction: dir,
                value_type: Catalog::type_ref(type_id),
                presence: conduit_core::Presence::Required,
                connections: conduit_core::ConnectionCardinality::ZeroOrMore,
                values: conduit_core::ValueCardinality::ExactlyOne,
                delivery: conduit_core::Delivery::Stream,
                temporal: conduit_core::TemporalContract::Committed,
                terminal: conduit_core::TerminalContract::Either,
                sensitivity: Sensitivity::Public,
                loss: conduit_core::LossAcceptance::LosslessOnly,
            }
        }
        let (inputs, outputs) = match id {
            "fixture/speech-node" => (
                vec![port(
                    "audio",
                    conduit_core::Direction::Input,
                    "fixture/text",
                )],
                vec![port(
                    "final",
                    conduit_core::Direction::Output,
                    "fixture/text",
                )],
            ),
            "fixture/incomplete-node" => (
                vec![port(
                    "audio",
                    conduit_core::Direction::Input,
                    "fixture/text",
                )],
                Vec::new(),
            ),
            "fixture/swapped-directions" => (
                vec![port(
                    "final",
                    conduit_core::Direction::Input,
                    "fixture/text",
                )],
                vec![port(
                    "audio",
                    conduit_core::Direction::Output,
                    "fixture/text",
                )],
            ),
            _ => return None,
        };
        Some(conduit_runtime::OwnedNodeContract {
            id: id.to_owned(),
            inputs,
            outputs,
        })
    }

    fn validate_literal(
        &self,
        expected: &OwnedTypeReference,
        source: &SourceValue,
    ) -> Result<OwnedSemanticValue, LiteralValidationError> {
        match (expected.id.as_str(), source) {
            ("fixture/bool", SourceValue::Boolean(value)) => {
                Ok(OwnedSemanticValue::Boolean(*value))
            }
            ("fixture/i8", SourceValue::Integer(value)) if i8::try_from(*value).is_ok() => {
                Ok(OwnedSemanticValue::Integer(*value))
            }
            ("fixture/i8", SourceValue::Integer(_)) => Err(LiteralValidationError::Overflow),
            ("fixture/text", SourceValue::Text(value)) => {
                Ok(OwnedSemanticValue::Text(value.clone()))
            }
            ("fixture/bytes", SourceValue::Bytes(value)) => {
                Ok(OwnedSemanticValue::Bytes(value.clone()))
            }
            ("fixture/ref", SourceValue::Reference(value))
            | ("fixture/contract-ref", SourceValue::ContractReference(value)) => {
                Ok(OwnedSemanticValue::Identifier(value.clone()))
            }
            ("fixture/list-bool", SourceValue::List(values)) => values
                .iter()
                .map(|value| self.validate_literal(&Self::type_ref("fixture/bool"), value))
                .collect::<Result<Vec<_>, _>>()
                .map(OwnedSemanticValue::List),
            ("fixture/decimal", SourceValue::ExactDecimal(value)) => {
                Ok(OwnedSemanticValue::Text(value.clone()))
            }
            ("fixture/record", SourceValue::Record(fields)) => {
                let mut lowered = Vec::new();
                for (key, value) in fields {
                    let expected = match key.as_str() {
                        "name" => Self::type_ref("fixture/text"),
                        "count" => Self::type_ref("fixture/i8"),
                        _ => return Err(LiteralValidationError::InvalidValue),
                    };
                    lowered.push((key.clone(), self.validate_literal(&expected, value)?));
                }
                Ok(OwnedSemanticValue::Map(lowered))
            }
            ("fixture/unavailable", _) => Err(LiteralValidationError::ProviderUnavailable),
            _ => Err(LiteralValidationError::WrongKind),
        }
    }

    fn validate_default(
        &self,
        expected: &OwnedTypeReference,
        value: &OwnedSemanticValue,
    ) -> Result<(), LiteralValidationError> {
        match (expected.id.as_str(), value) {
            ("fixture/bool", OwnedSemanticValue::Boolean(_))
            | ("fixture/i8", OwnedSemanticValue::Integer(_))
            | ("fixture/text" | "fixture/decimal", OwnedSemanticValue::Text(_))
            | ("fixture/bytes", OwnedSemanticValue::Bytes(_))
            | ("fixture/ref" | "fixture/contract-ref", OwnedSemanticValue::Identifier(_))
            | ("fixture/list-bool", OwnedSemanticValue::List(_))
            | ("fixture/record", OwnedSemanticValue::Map(_)) => Ok(()),
            ("fixture/unavailable", _) => Err(LiteralValidationError::ProviderUnavailable),
            _ => Err(LiteralValidationError::WrongKind),
        }
    }
}

#[test]
fn current_grammar_uses_the_current_source_ast_schema() {
    let lowered = lower_source(
        &graph("panel 0\ninterface fixture/ports { > value : fixture/text }\n"),
        &Catalog,
    )
    .unwrap();
    assert_eq!(
        lowered.source_ast_schema_version,
        conduit_runtime::SOURCE_AST_SCHEMA_VERSION
    );
}

struct OrderedPortCatalog(Vec<String>);

impl SourceContractCatalog for OrderedPortCatalog {
    fn node_schema(&self, _id: &str) -> Option<OwnedNodeSchema> {
        None
    }

    fn type_reference(&self, _id: &str) -> Option<OwnedTypeReference> {
        None
    }

    fn port_contract(&self, id: &str) -> Option<OwnedPortReference> {
        self.0
            .iter()
            .find(|candidate| candidate.as_str() == id)
            .and_then(|candidate| Catalog.port_contract(candidate))
    }

    fn validate_literal(
        &self,
        _expected: &OwnedTypeReference,
        _source: &SourceValue,
    ) -> Result<OwnedSemanticValue, LiteralValidationError> {
        unreachable!("port-group-only catalog has no source literals")
    }

    fn validate_default(
        &self,
        _expected: &OwnedTypeReference,
        _value: &OwnedSemanticValue,
    ) -> Result<(), LiteralValidationError> {
        unreachable!("port-group-only catalog has no defaults")
    }
}

fn graph(source: &str) -> conduit_panel::ModuleGraph {
    resolve_modules(
        "mem://fixture/root.panel",
        None,
        &MemoryLoader(BTreeMap::from([(
            "mem://fixture/root.panel".to_owned(),
            source.to_owned(),
        )])),
    )
    .unwrap()
}

#[test]
fn hosted_schemas_copy_core_contract_facts_and_hash_every_exact_change() {
    const TYPE: TypeContractRef<'static> = TypeContractRef {
        contract_id: Id("fixture/bool"),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([0x11; 32]),
    };
    const FIELD: ConfigFieldContract<'static> = ConfigFieldContract {
        key: Id("enabled"),
        value_type: TYPE,
        requirement: ConfigRequirement::Defaulted(CanonicalValue::Boolean(true)),
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    };
    const CONTRACT: NodeContract<'static> = NodeContract {
        id: Id("fixture/defaulted"),
        config: ConfigContract { fields: &[FIELD] },
        inputs: &[],
        outputs: &[],
    };
    let copied = OwnedNodeSchema::from_contract(&CONTRACT);
    assert_eq!(copied.fields[0].value_type.id, "fixture/bool");
    assert!(matches!(
        copied.fields[0].requirement,
        OwnedConfigRequirement::Defaulted(OwnedSemanticValue::Boolean(true))
    ));

    let mut changed = copied.clone();
    changed.fields[0].value_type.semantic_hash = SemanticHash::from_bytes([0x22; 32]);
    assert_ne!(copied.semantic_hash(), changed.semantic_hash());
}

fn fixture_graph(case: &Value, source_field: &str) -> Result<conduit_panel::ModuleGraph, String> {
    fixture_graph_with_root(case, source_field, None)
}

fn fixture_graph_with_root(
    case: &Value,
    source_field: &str,
    selected_root: Option<&str>,
) -> Result<conduit_panel::ModuleGraph, String> {
    let entry_uri = case
        .get("entry_uri")
        .and_then(Value::as_str)
        .unwrap_or("mem://fixture/root.panel");
    let source = case[source_field]
        .as_str()
        .ok_or_else(|| format!("{source_field} is absent"))?;
    let mut modules = BTreeMap::from([(entry_uri.to_owned(), source.to_owned())]);
    if let Some(imports) = case.get("modules").and_then(Value::as_object) {
        modules.extend(imports.iter().map(|(uri, source)| {
            (
                uri.clone(),
                source.as_str().expect("module source is text").to_owned(),
            )
        }));
    }
    resolve_modules(entry_uri, selected_root, &MemoryLoader(modules))
        .map_err(|error| error.code.to_owned())
}

fn relation(left: SemanticHash, right: SemanticHash) -> &'static str {
    if left == right { "equal" } else { "different" }
}

fn text_relation(left: &str, right: &str) -> &'static str {
    if left == right { "equal" } else { "different" }
}

fn root_mode(mode: RootSelectionMode) -> &'static str {
    match mode {
        RootSelectionMode::Explicit => "explicit",
        RootSelectionMode::ImplicitSole => "implicit-sole",
    }
}

#[test]
fn every_normative_source_lowering_vector_has_the_exact_result() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/c3/source-lowering.json")).unwrap();
    assert_eq!(fixture["source_ast_schema_version"], 0);
    assert_eq!(fixture["lowering_schema_version"], 0);

    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let expected = &case["expected"];
        let actual = match case["assertion"].as_str().unwrap() {
            "source-root-selection" => {
                let roots = case["selected_roots"].as_array().unwrap();
                let first = fixture_graph_with_root(case, "source", roots[0].as_str()).unwrap();
                let second = fixture_graph_with_root(case, "source", roots[1].as_str()).unwrap();
                let first = &first.modules.last().unwrap().panel;
                let second = &second.modules.last().unwrap().panel;
                json!({
                    "relation": text_relation(
                        &semantic_source_hash(first),
                        &semantic_source_hash(second),
                    ),
                })
            }
            "lowering-root-selection" => {
                let roots = case["selected_roots"].as_array().unwrap();
                let first = lower_topology(
                    &fixture_graph_with_root(case, "source", roots[0].as_str()).unwrap(),
                    &Catalog,
                )
                .unwrap();
                let second = lower_topology(
                    &fixture_graph_with_root(case, "source", roots[1].as_str()).unwrap(),
                    &Catalog,
                )
                .unwrap();
                json!({
                    "relation": relation(first.semantic_hash, second.semantic_hash),
                    "targets": [
                        first.root_selection.unwrap().target,
                        second.root_selection.unwrap().target,
                    ],
                })
            }
            "implicit-explicit-root" => {
                let implicit =
                    lower_topology(&fixture_graph(case, "source").unwrap(), &Catalog).unwrap();
                let explicit = lower_topology(
                    &fixture_graph_with_root(case, "source", Some("app")).unwrap(),
                    &Catalog,
                )
                .unwrap();
                let implicit_selection = implicit.root_selection.unwrap();
                let explicit_selection = explicit.root_selection.unwrap();
                json!({
                    "relation": relation(implicit.semantic_hash, explicit.semantic_hash),
                    "implicit_mode": root_mode(implicit_selection.mode),
                    "explicit_mode": root_mode(explicit_selection.mode),
                })
            }
            "compare" => {
                let first_graph = fixture_graph(case, "source").unwrap();
                let second_graph = fixture_graph(case, "comparison_source").unwrap();
                let first_panel = &first_graph.modules.last().unwrap().panel;
                let second_panel = &second_graph.modules.last().unwrap().panel;
                let first = lower_topology(&first_graph, &Catalog).unwrap();
                let second = lower_topology(&second_graph, &Catalog).unwrap();
                json!({
                    "source_relation": text_relation(
                        &semantic_source_hash(first_panel),
                        &semantic_source_hash(second_panel),
                    ),
                    "lowering_relation": relation(first.semantic_hash, second.semantic_hash),
                    "content_relation": text_relation(
                        &first_graph.modules.last().unwrap().content_hash,
                        &second_graph.modules.last().unwrap().content_hash,
                    ),
                })
            }
            "constraint" => {
                let first =
                    lower_topology(&fixture_graph(case, "source").unwrap(), &Catalog).unwrap();
                let second =
                    lower_topology(&fixture_graph(case, "comparison_source").unwrap(), &Catalog)
                        .unwrap();
                let constraint_origin = first
                    .source_map
                    .iter()
                    .find(|entry| entry.semantic_path.ends_with("/constraint"))
                    .and_then(|entry| entry.origins.first())
                    .unwrap();
                json!({
                    "constraint": first.nodes[0].unresolved_constraint,
                    "relation": relation(first.semantic_hash, second.semantic_hash),
                    "origin_line": constraint_origin.span.line,
                    "origin_column": constraint_origin.span.column,
                })
            }
            "cord" => {
                let first =
                    lower_topology(&fixture_graph(case, "source").unwrap(), &Catalog).unwrap();
                let second =
                    lower_topology(&fixture_graph(case, "comparison_source").unwrap(), &Catalog)
                        .unwrap();
                json!({
                    "cords": first.cords.len(),
                    "capacity": first.cords[0].capacity_items,
                    "relation": relation(first.semantic_hash, second.semantic_hash),
                    "origin_line": first.cords[0].origin.span.line,
                })
            }
            "export" => {
                let first =
                    lower_topology(&fixture_graph(case, "source").unwrap(), &Catalog).unwrap();
                let second =
                    lower_topology(&fixture_graph(case, "comparison_source").unwrap(), &Catalog)
                        .unwrap();
                json!({
                    "composites": first.composites.len(),
                    "children": first.composite_children.len(),
                    "exports": first.exports.len(),
                    "target_ends": first.exports[0]
                        .target
                        .strip_prefix("mem://fixture/root.panel/definition/box")
                        .unwrap(),
                    "relation": relation(first.semantic_hash, second.semantic_hash),
                    "origin_line": first.exports[0].origin.span.line,
                })
            }
            "binding" => {
                let first =
                    lower_topology(&fixture_graph(case, "source").unwrap(), &Catalog).unwrap();
                let second =
                    lower_topology(&fixture_graph(case, "comparison_source").unwrap(), &Catalog)
                        .unwrap();
                json!({
                    "composites": first.composites.len(),
                    "bindings": first.bindings.len(),
                    "target_ends": first.bindings[0]
                        .target
                        .strip_prefix("mem://fixture/root.panel/definition/box")
                        .unwrap(),
                    "relation": relation(first.semantic_hash, second.semantic_hash),
                    "origin_line": first.bindings[0].origin.span.line,
                })
            }
            "imported-topology" => {
                let lowered =
                    lower_topology(&fixture_graph(case, "source").unwrap(), &Catalog).unwrap();
                let child_export = lowered
                    .exports
                    .iter()
                    .find(|export| export.origin.module_uri == "mem://fixture/child.panel")
                    .unwrap();
                json!({
                    "composites": lowered.composites.len(),
                    "children": lowered.composite_children.len(),
                    "cords": lowered.cords.len(),
                    "exports": lowered.exports.len(),
                    "constraints": lowered
                        .nodes
                        .iter()
                        .filter(|node| node.unresolved_constraint.is_some())
                        .count(),
                    "child_origin_uri": child_export.origin.module_uri,
                    "child_export_line": child_export.origin.span.line,
                })
            }
            "v1-preservation"
            | "migration"
            | "stale-migration"
            | "ambiguous-migration"
            | "unsupported-lowering-version" => continue,
            "parse-diagnostic" => {
                let error = conduit_panel::parse(case["source"].as_str().unwrap()).unwrap_err();
                json!({"outcome": "rejected", "code": error.code})
            }
            assertion => panic!("{id}: unknown assertion {assertion}"),
        };
        assert_eq!(actual, *expected, "{id}");
    }
}

#[test]
fn every_normative_port_group_source_vector_has_the_exact_result() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c2/port-group-correlation.json"
    ))
    .unwrap();
    for case in fixture["port_group_cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let expected = case["expected"].clone();
        let source = case["source"].as_str().unwrap();
        let actual = match case["assertion"].as_str().unwrap() {
            "parse-diagnostic" => {
                let error = conduit_panel::parse(source).unwrap_err();
                json!({"outcome": "rejected", "code": error.code})
            }
            "parse" => {
                let panel = conduit_panel::parse(source).unwrap();
                let target = panel
                    .definitions
                    .iter()
                    .flat_map(|definition| &definition.exports)
                    .find(|export| export.target.port.contains('['))
                    .map(|export| export.target.port.as_str())
                    .unwrap();
                json!({"outcome": "accepted", "target": target})
            }
            "diagnostic" => {
                let error = lower_source_base(&graph(source), &Catalog).unwrap_err();
                json!({"outcome": "rejected", "code": error.code})
            }
            "compare" => {
                let first = lower_source_base(&graph(source), &Catalog).unwrap();
                let second = lower_source_base(
                    &graph(case["comparison_source"].as_str().unwrap()),
                    &Catalog,
                )
                .unwrap();
                json!({
                    "outcome": "accepted",
                    "relation": if first.semantic_hash == second.semantic_hash {
                        "equal"
                    } else {
                        "different"
                    }
                })
            }
            "catalog-order" => {
                let orders = case["catalog_orders"].as_array().unwrap();
                let catalog = |order: &Value| {
                    OrderedPortCatalog(
                        order
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| value.as_str().unwrap().to_owned())
                            .collect(),
                    )
                };
                let first = lower_source_base(&graph(source), &catalog(&orders[0])).unwrap();
                let second = lower_source_base(&graph(source), &catalog(&orders[1])).unwrap();
                json!({
                    "outcome": "accepted",
                    "relation": if first.semantic_hash == second.semantic_hash {
                        "equal"
                    } else {
                        "different"
                    }
                })
            }
            "lower" => {
                let lowered = lower_source_base(&graph(source), &Catalog).unwrap();
                let members: Vec<Value> = lowered
                    .group_ports
                    .iter()
                    .map(|member| {
                        if let Some(member_origin) = &member.member_origin {
                            let span = &member_origin.span;
                            json!({
                                "key": member.member,
                                "ordinal": member.ordinal,
                                "line": span.line,
                                "column": span.column,
                                "end_column": span.end_column,
                            })
                        } else {
                            json!({
                                "key": member.member,
                                "ordinal": member.ordinal,
                                "authored_span": false,
                            })
                        }
                    })
                    .collect();
                json!({
                    "outcome": "accepted",
                    "maximum": lowered.group_ports[0].group_maximum,
                    "members": members,
                })
            }
            other => panic!("{id}: unknown port-group assertion {other}"),
        };
        assert_eq!(actual, expected, "{id}");
    }
    let families = fixture["identity_families"].as_array().unwrap();
    let mut family_ids = std::collections::BTreeSet::new();
    for family in families {
        let id = family["id"].as_str().unwrap();
        assert!(family_ids.insert(id), "duplicate identity family {id}");
        for field in [
            "allocator",
            "scope",
            "lifetime",
            "uniqueness",
            "serialization",
            "sensitivity",
            "propagation",
        ] {
            assert!(
                family[field]
                    .as_str()
                    .is_some_and(|value| !value.is_empty()),
                "{id}: missing {field}"
            );
        }
    }
    for case in fixture["propagation_cases"].as_array().unwrap() {
        let preserved: std::collections::BTreeSet<_> = case["preserve"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        for field in ["allocate", "replace"] {
            if let Some(values) = case.get(field).and_then(Value::as_array) {
                assert!(
                    values
                        .iter()
                        .map(|value| value.as_str().unwrap())
                        .all(|value| !preserved.contains(value)),
                    "{}: {field} overlaps preserved identity",
                    case["id"]
                );
            }
        }
    }
    for case in fixture["negative_allocator_cases"].as_array().unwrap() {
        assert_eq!(case["expected"]["outcome"], "rejected", "{}", case["id"]);
    }
}

#[test]
fn explicit_and_default_values_have_one_descriptor_with_visible_provenance() {
    let omitted = lower_source_base(
        &graph("panel 0\nnode value : fixture/defaulted\n"),
        &Catalog,
    )
    .unwrap();
    let explicit = lower_source_base(
        &graph("panel 0\nnode value : fixture/defaulted { enabled = true }\n"),
        &Catalog,
    )
    .unwrap();
    assert_eq!(omitted.semantic_hash, explicit.semantic_hash);
    assert_eq!(
        omitted.nodes[0].config[0].provenance,
        ConfigProvenance::SchemaDefault
    );
    assert_eq!(
        explicit.nodes[0].config[0].provenance,
        ConfigProvenance::Authored
    );
    assert!(
        omitted
            .source_map
            .iter()
            .any(|entry| entry.semantic_path.ends_with("/config/enabled"))
    );
    assert!(omitted.explain().contains("provenance=schema-default"));
}

#[test]
fn records_are_canonical_and_precision_sensitive_values_remain_exact() {
    let left = lower_source_base(
        &graph("panel 0\nnode value : fixture/record { value = record(name=\"a\",count=7) }\n"),
        &Catalog,
    )
    .unwrap();
    let right = lower_source_base(
        &graph("panel 0\nnode value : fixture/record { value = record(count=7,name=\"a\") }\n"),
        &Catalog,
    )
    .unwrap();
    assert_eq!(left.semantic_hash, right.semantic_hash);

    let short = lower_source_base(
        &graph("panel 0\nnode value : fixture/decimal { value = decimal(\"0.1\") }\n"),
        &Catalog,
    )
    .unwrap();
    let precise = lower_source_base(
        &graph(
            "panel 0\nnode value : fixture/decimal { value = decimal(\"0.10000000000000001\") }\n",
        ),
        &Catalog,
    )
    .unwrap();
    assert_ne!(short.semantic_hash, precise.semantic_hash);
}

#[test]
fn wrong_types_overflow_and_missing_providers_name_span_and_contract() {
    for (source, code) in [
        (
            "panel 0\nnode value : fixture/integer { count = \"seven\" }\n",
            "CND-LWR-005",
        ),
        (
            "panel 0\nnode value : fixture/integer { count = 128 }\n",
            "CND-LWR-006",
        ),
        (
            "panel 0\nnode value : fixture/provider { value = \"x\" }\n",
            "CND-LWR-008",
        ),
    ] {
        let error = lower_source_base(&graph(source), &Catalog).unwrap_err();
        assert_eq!(error.code, code);
        assert!(error.expected_contract.is_some());
        let origin = error.origin.expect("authored value span");
        assert_eq!(origin.module_uri, "mem://fixture/root.panel");
        assert_eq!(origin.span.line, 2);
        assert!(origin.span.end_column > origin.span.column);
    }
}

#[test]
fn diagnostic_value_spans_are_exact_and_exclude_following_trivia() {
    let error = lower_source_base(
        &graph("panel 0\nnode n : fixture/integer { count = -129    }\n"),
        &Catalog,
    )
    .unwrap_err();
    let origin = error.origin.unwrap();
    assert_eq!(origin.span.line, 2);
    assert_eq!(origin.span.column, 36);
    assert_eq!(origin.span.end_line, 2);
    assert_eq!(origin.span.end_column, 40);
}

#[test]
fn secret_references_are_plan_bindings_and_never_format_the_reference() {
    let lowered = lower_source_base(
        &graph("panel 0\nnode value : fixture/secret { token = secret(\"do-not-print-this\") }\n"),
        &Catalog,
    )
    .unwrap();
    assert!(matches!(
        lowered.nodes[0].config[0].value,
        LoweredConfigValue::SecretReference(_)
    ));
    assert_eq!(
        lowered.nodes[0].config[0].provenance,
        ConfigProvenance::PlanBinding
    );
    assert!(!format!("{lowered:?}").contains("do-not-print-this"));
    assert!(!lowered.explain().contains("do-not-print-this"));
    assert!(lowered.explain().contains("[REDACTED]"));
}

#[test]
fn imported_definition_schema_and_multi_file_origins_remain_exact() {
    let child = "panel 0\nnode configured(count: fixture/i8) { }\nroot configured\n";
    let entry = "panel 0\nimport \"./child.panel\" as child\n\
                 node app : child.configured { count = 7 }\n";
    let graph = resolve_modules(
        "mem://fixture/root.panel",
        None,
        &MemoryLoader(BTreeMap::from([
            ("mem://fixture/root.panel".to_owned(), entry.to_owned()),
            ("mem://fixture/child.panel".to_owned(), child.to_owned()),
        ])),
    )
    .unwrap();
    let lowered = lower_source_base(&graph, &Catalog).unwrap();
    let app = lowered
        .nodes
        .iter()
        .find(|node| node.path.ends_with("/node/app"))
        .unwrap();
    assert!(app.contract_id.contains("#configured"));
    let origin = app.config[0].origin.as_ref().unwrap();
    assert_eq!(origin.module_uri, "mem://fixture/root.panel");
    assert_eq!(origin.span.line, 3);
    assert!(
        lowered
            .source_map
            .iter()
            .any(|entry| entry.semantic_path.ends_with("/node/app/config/count"))
    );
}

#[test]
fn groups_and_pools_lower_to_finite_plan_visible_specs() {
    let lowered = lower_source_base(
        &graph(
            "panel 0\n\
             port-group > routes : fixture/request-port keyed max 2 { member home member assets }\n\
             port-group workers > : fixture/reply-port indexed max 3\n\
             pool sessions : fixture/handler { maximum = 8 admission = queue_bounded admission_queue = 16 deadline_ms = 1000 idle_timeout_ms = 5000 supervision = restart_bounded restart_attempts = 2 restart_backoff_ms = 50 cleanup = drain }\n",
        ),
        &Catalog,
    )
    .unwrap();
    assert_eq!(lowered.group_ports.len(), 5);
    assert_eq!(lowered.group_ports[0].group_id, "routes");
    assert_eq!(lowered.group_ports[0].member, "home");
    assert_eq!(lowered.group_ports[0].group_maximum, 2);
    assert_eq!(
        lowered.group_ports[0].direction,
        conduit_panel::ExportDirection::Input
    );
    assert_eq!(lowered.pools.len(), 1);
    assert_eq!(lowered.pools[0].maximum, 8);
    assert!(matches!(
        lowered.pools[0].admission,
        conduit_panel::PoolAdmission::QueueBounded(16)
    ));
    assert!(
        lowered
            .source_map
            .iter()
            .all(|entry| !entry.origins.is_empty())
    );
}

#[test]
fn every_normative_panel_interface_satisfaction_vector_has_the_exact_result() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/c3/panel-interface-satisfaction.json"
    ))
    .unwrap();
    assert_eq!(fixture["source_ast_schema_version"], 0);
    assert_eq!(fixture["lowering_schema_version"], 0);

    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let expected = &case["expected"];
        let actual = match case["assertion"].as_str().unwrap() {
            "diagnostic" => {
                let graph = fixture_graph(case, "source").unwrap();
                match conduit_runtime::lower_source(&graph, &Catalog) {
                    Ok(_) => json!({"outcome": "accepted"}),
                    Err(error) => json!({"outcome": "rejected", "code": error.code}),
                }
            }
            assertion => panic!("{id}: unknown assertion {assertion}"),
        };
        assert_eq!(actual, *expected, "{id}");
    }
}
