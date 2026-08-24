use std::collections::BTreeMap;

use conduit_form::{
    parse_syntax_document, ConstructionRole, ConstructionSyntax, ExpressionSyntax,
    StructuredExpressionField,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Number, Value};
use std::fmt::Write as _;

use crate::{
    BodyBindingTarget, BodyDescription, BodyDescriptionDiagnostic, BodyHostDescription,
    ConfigurationBase, ConfigurationDiagnostic, ConfigurationTarget, HostBounds, HostConfiguration,
    ResourceBudget,
};

pub fn parse_host_configuration_conduit(
    source: &str,
) -> Result<HostConfiguration, ConfigurationDiagnostic> {
    let construction = one_construction(source, ConstructionRole::Host)
        .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    let declarations = declarations(&construction);
    let schema = one_required::<u32>(&declarations, "schema")
        .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    let target = one_required::<ConfigurationTarget>(&declarations, "target")
        .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    let limits = one_required::<HostBounds>(&declarations, "limits")
        .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    let bases = repeated::<ConfigurationBase>(&declarations, "base")
        .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    let resources = repeated::<ResourceBudget>(&declarations, "need")
        .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    reject_unknown(
        &declarations,
        &["schema", "target", "limits", "base", "need"],
    )
    .map_err(|detail| ConfigurationDiagnostic::Decode { detail })?;
    Ok(HostConfiguration {
        schema,
        name: construction.name.text.clone(),
        target,
        bases,
        resources,
        limits,
    })
}

pub fn canonical_host_configuration_conduit(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let mut canonical = configuration.clone();
    canonical
        .bases
        .sort_by(|left, right| left.kind.cmp(&right.kind));
    canonical
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    let mut source = String::new();
    writeln!(&mut source, "host {} {{", canonical.name).map_err(|error| {
        ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        }
    })?;
    writeln!(&mut source, "  schema = {}", canonical.schema).map_err(|error| {
        ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        }
    })?;
    write!(
        &mut source,
        "  target = {{architecture: {}, machine: {}",
        string(&canonical.target.architecture)?,
        string(&canonical.target.machine)?
    )
    .map_err(|error| ConfigurationDiagnostic::Encode {
        detail: error.to_string(),
    })?;
    if let Some(board) = &canonical.target.board {
        write!(&mut source, ", board: {}", string(board)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
    }
    if let Some(os) = &canonical.target.os {
        write!(&mut source, ", os: {}", string(os)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
    }
    source.push_str("}\n");
    for base in &canonical.bases {
        write!(&mut source, "  base = {{kind: {}", string(&base.kind)?).map_err(|error| {
            ConfigurationDiagnostic::Encode {
                detail: error.to_string(),
            }
        })?;
        if let Some(implementation) = &base.implementation {
            write!(&mut source, ", implementation: {}", string(implementation)?).map_err(
                |error| ConfigurationDiagnostic::Encode {
                    detail: error.to_string(),
                },
            )?;
        }
        if !base.implementations.is_empty() {
            let implementations = base
                .implementations
                .iter()
                .map(|item| string(item))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            write!(&mut source, ", implementations: [{implementations}]").map_err(|error| {
                ConfigurationDiagnostic::Encode {
                    detail: error.to_string(),
                }
            })?;
        }
        source.push_str("}\n");
    }
    for need in &canonical.resources {
        writeln!(
            &mut source,
            "  need = {{id: {}, class: {}, slots: {}, bytes: {}}}",
            string(&need.id)?,
            string(&need.class)?,
            need.slots,
            need.bytes
        )
        .map_err(|error| ConfigurationDiagnostic::Encode {
            detail: error.to_string(),
        })?;
    }
    let limits = &canonical.limits;
    writeln!(
        &mut source,
        "  limits = {{static_memory_bytes: {}, heap_arena_bytes: {}, queue_items: {}, buffered_bytes: {}, active_instances: {}, operation_slots: {}, timer_slots: {}, line_sessions: {}, evidence_items: {}}}",
        limits.static_memory_bytes,
        limits.heap_arena_bytes,
        limits.queue_items,
        limits.buffered_bytes,
        limits.active_instances,
        limits.operation_slots,
        limits.timer_slots,
        limits.line_sessions,
        limits.evidence_items
    )
    .map_err(|error| ConfigurationDiagnostic::Encode { detail: error.to_string() })?;
    source.push_str("}\n");
    Ok(source)
}

pub fn parse_body_description_conduit(
    source: &str,
) -> Result<BodyDescription, BodyDescriptionDiagnostic> {
    let construction = one_construction(source, ConstructionRole::Body)
        .map_err(|detail| BodyDescriptionDiagnostic::Decode { detail })?;
    let declarations = declarations(&construction);
    let schema = one_required::<u32>(&declarations, "schema")
        .map_err(|detail| BodyDescriptionDiagnostic::Decode { detail })?;
    let id = one_required::<String>(&declarations, "id")
        .map_err(|detail| BodyDescriptionDiagnostic::Decode { detail })?;
    let hosts = repeated::<BodyHostDescription>(&declarations, "host")
        .map_err(|detail| BodyDescriptionDiagnostic::Decode { detail })?;
    reject_unknown(&declarations, &["schema", "id", "host"])
        .map_err(|detail| BodyDescriptionDiagnostic::Decode { detail })?;
    Ok(BodyDescription {
        schema,
        name: construction.name.text.clone(),
        body: BodyBindingTarget { id },
        hosts,
    })
}

fn one_construction(
    source: &str,
    expected: ConstructionRole,
) -> Result<ConstructionSyntax, String> {
    let document = parse_syntax_document(source);
    if let Some(diagnostic) = document.diagnostics.first() {
        return Err(format!(
            "{} at {}:{}",
            diagnostic.message, diagnostic.span.line, diagnostic.span.column
        ));
    }
    if !document.forms.is_empty() {
        return Err("construction source must not contain Form definitions".into());
    }
    if document.constructions.len() != 1 {
        return Err("construction source must contain exactly one document".into());
    }
    let construction = document
        .constructions
        .into_iter()
        .next()
        .expect("one construction was required");
    if construction.role != expected {
        return Err(format!(
            "expected a {} document",
            match expected {
                ConstructionRole::Host => "host",
                ConstructionRole::Body => "body",
            }
        ));
    }
    Ok(construction)
}

fn declarations(construction: &ConstructionSyntax) -> BTreeMap<&str, Vec<&ExpressionSyntax>> {
    let mut values = BTreeMap::<&str, Vec<&ExpressionSyntax>>::new();
    for declaration in &construction.declarations {
        values
            .entry(declaration.name.text.as_str())
            .or_default()
            .push(&declaration.value.syntax);
    }
    values
}

fn one_required<T: DeserializeOwned>(
    declarations: &BTreeMap<&str, Vec<&ExpressionSyntax>>,
    name: &str,
) -> Result<T, String> {
    let Some(values) = declarations.get(name) else {
        return Err(format!("missing required '{name}' declaration"));
    };
    let [value] = values.as_slice() else {
        return Err(format!("duplicate conflicting '{name}' declaration"));
    };
    decode(value).map_err(|detail| format!("invalid '{name}' declaration: {detail}"))
}

fn repeated<T: DeserializeOwned>(
    declarations: &BTreeMap<&str, Vec<&ExpressionSyntax>>,
    name: &str,
) -> Result<Vec<T>, String> {
    declarations
        .get(name)
        .into_iter()
        .flat_map(|values| values.iter())
        .map(|value| {
            decode(value).map_err(|detail| format!("invalid '{name}' declaration: {detail}"))
        })
        .collect()
}

fn reject_unknown(
    declarations: &BTreeMap<&str, Vec<&ExpressionSyntax>>,
    allowed: &[&str],
) -> Result<(), String> {
    if let Some(name) = declarations.keys().find(|name| !allowed.contains(name)) {
        return Err(format!("unknown construction declaration '{name}'"));
    }
    Ok(())
}

fn decode<T: DeserializeOwned>(syntax: &ExpressionSyntax) -> Result<T, String> {
    serde_json::from_value(value(syntax)?).map_err(|error| error.to_string())
}

fn value(syntax: &ExpressionSyntax) -> Result<Value, String> {
    match syntax {
        ExpressionSyntax::Atomic(atom) => atomic(&atom.text),
        ExpressionSyntax::Collection { values, .. } => values
            .iter()
            .map(value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        ExpressionSyntax::Record { fields, .. } => record(fields),
        ExpressionSyntax::Variant { tag, .. } => Err(format!(
            "variant '{}' is not a construction value",
            tag.text
        )),
    }
}

fn record(fields: &[StructuredExpressionField]) -> Result<Value, String> {
    let mut object = Map::new();
    for field in fields {
        if object.contains_key(&field.name.text) {
            return Err(format!("duplicate record field '{}'", field.name.text));
        }
        object.insert(field.name.text.clone(), value(&field.value)?);
    }
    Ok(Value::Object(object))
}

fn atomic(text: &str) -> Result<Value, String> {
    if text.starts_with('"') {
        return serde_json::from_str::<String>(text)
            .map(Value::String)
            .map_err(|error| error.to_string());
    }
    if text.starts_with('\'') && text.ends_with('\'') && text.len() >= 2 {
        return Ok(Value::String(text[1..text.len() - 1].to_string()));
    }
    match text {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    if let Ok(value) = text.parse::<u64>() {
        return Ok(Value::Number(Number::from(value)));
    }
    Err(format!(
        "'{text}' must be a quoted string, boolean, or unsigned integer"
    ))
}

fn string(value: &str) -> Result<String, ConfigurationDiagnostic> {
    serde_json::to_string(value).map_err(|error| ConfigurationDiagnostic::Encode {
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canonical_profile_json, check_body_description, check_host_configuration,
        parse_body_description, parse_host_configuration, ConfigurationDiagnostic,
        FabricationCatalog,
    };

    #[test]
    fn canonical_host_sources_match_toml_semantics_and_identity() {
        let pairs = [
            (
                include_str!(
                    "../../../profiles/host-configurations/linux-workstation.host.conduit"
                ),
                include_str!("../../../profiles/host-configurations/linux-workstation.host.toml"),
            ),
            (
                include_str!("../../../profiles/host-configurations/pico-w.host.conduit"),
                include_str!("../../../profiles/host-configurations/pico-w.host.toml"),
            ),
        ];
        let catalog = FabricationCatalog::canonical();
        for (conduit, toml) in pairs {
            let canonical = check_host_configuration(
                parse_host_configuration_conduit(conduit).unwrap(),
                &catalog,
            )
            .unwrap();
            let migration =
                check_host_configuration(parse_host_configuration(toml).unwrap(), &catalog)
                    .unwrap();
            assert_eq!(canonical.configuration(), migration.configuration());
            assert_eq!(canonical.configuration_id(), migration.configuration_id());
            assert_eq!(
                canonical_profile_json(canonical.profile()).unwrap(),
                canonical_profile_json(migration.profile()).unwrap()
            );
            let encoded = canonical_host_configuration_conduit(canonical.configuration()).unwrap();
            assert_eq!(
                parse_host_configuration_conduit(&encoded).unwrap(),
                canonical.configuration().clone()
            );
        }
    }

    #[test]
    fn need_declarations_lower_to_finite_existing_resource_budgets() {
        let source = include_str!(
            "../../../profiles/host-configurations/linux-workstation.host.conduit"
        )
        .replace(
            "  limits =",
            "  need = {id: \"need:model-memory\", class: \"memory\", slots: 1, bytes: 4096}\n  limits =",
        );
        let configuration = parse_host_configuration_conduit(&source).unwrap();
        assert_eq!(configuration.resources.len(), 1);
        assert_eq!(configuration.resources[0].id, "need:model-memory");
        assert_eq!(configuration.resources[0].bytes, 4096);
        check_host_configuration(configuration, &FabricationCatalog::canonical()).unwrap();
    }

    #[test]
    fn canonical_and_migration_inputs_share_host_validation_truth() {
        let canonical =
            include_str!("../../../profiles/host-configurations/linux-workstation.host.conduit");
        let migration =
            include_str!("../../../profiles/host-configurations/linux-workstation.host.toml");
        let cases = [
            ("x86_64", "mystery", "UnknownTarget"),
            ("clock/monotonic", "unknown/base", "UnknownBase"),
            (
                "hosted/monotonic-clock@1",
                "unknown-driver@1",
                "UnknownImplementation",
            ),
            (
                "hosted/monotonic-clock@1",
                "pico/usb-cdc@1",
                "IncompatibleImplementation",
            ),
            ("queue_items: 4096", "queue_items: 0", "UnboundedCapacity"),
        ];
        for (from, to, expected) in cases {
            let conduit = canonical.replace(from, to);
            let toml = migration.replace(from, to).replace(
                "queue_items = 4096",
                if expected == "UnboundedCapacity" {
                    "queue_items = 0"
                } else {
                    "queue_items = 4096"
                },
            );
            let canonical_diagnostics = check_host_configuration(
                parse_host_configuration_conduit(&conduit).unwrap(),
                &FabricationCatalog::canonical(),
            )
            .unwrap_err();
            let migration_diagnostics = check_host_configuration(
                parse_host_configuration(&toml).unwrap(),
                &FabricationCatalog::canonical(),
            )
            .unwrap_err();
            assert!(format!("{canonical_diagnostics:?}").contains(expected));
            assert!(format!("{migration_diagnostics:?}").contains(expected));
        }

        let duplicate = canonical.replace(
            "  limits =",
            "  base = {kind: \"clock/monotonic\", implementation: \"hosted/protected-file@1\"}\n  limits =",
        );
        let diagnostics = check_host_configuration(
            parse_host_configuration_conduit(&duplicate).unwrap(),
            &FabricationCatalog::canonical(),
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|item| matches!(
            item,
            ConfigurationDiagnostic::DuplicateContradictoryBase { .. }
        )));
    }

    #[test]
    fn canonical_body_composes_canonical_hosts_through_existing_model() {
        let source = include_str!("../../../profiles/bodies/pete-r1.body.conduit");
        let description = parse_body_description_conduit(source).unwrap();
        let migration =
            parse_body_description(include_str!("../../../profiles/bodies/pete-r1.body.toml"))
                .unwrap();
        assert_eq!(description.schema, migration.schema);
        assert_eq!(description.name, migration.name);
        assert_eq!(description.body, migration.body);

        let mut configurations = BTreeMap::new();
        for host in &description.hosts {
            let source = match host.name.as_str() {
                "forebrain" => include_str!(
                    "../../../profiles/host-configurations/linux-workstation.host.conduit"
                ),
                "brainstem" => {
                    include_str!("../../../profiles/host-configurations/pico-w.host.conduit")
                }
                "eyes" => {
                    include_str!("../../../profiles/host-configurations/browser-page.host.conduit")
                }
                _ => unreachable!(),
            };
            configurations.insert(
                host.configuration.clone(),
                parse_host_configuration_conduit(source).unwrap(),
            );
        }
        let checked = check_body_description(
            description,
            &configurations,
            &FabricationCatalog::canonical(),
        )
        .unwrap();
        assert_eq!(checked.hosts().len(), 3);
    }

    #[test]
    fn user_facing_commands_name_only_the_canonical_source_family() {
        for document in [
            include_str!("../../../README.md"),
            include_str!("../../../docs/host-fabrication.md"),
            include_str!("../../../docs/body-building.md"),
        ] {
            for line in document.lines().filter(|line| line.contains("cargo xtask")) {
                assert!(
                    !line.contains(".toml"),
                    "ordinary authoring entrance leaked a migration format: {line}"
                );
            }
        }
    }
}
