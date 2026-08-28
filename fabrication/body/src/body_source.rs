use std::fmt::Write as _;

use crate::{BodyDescription, BodyDescriptionDiagnostic};

/// Encode one Body description as deterministic canonical Conduit construction source.
pub fn canonical_body_description_conduit(
    description: &BodyDescription,
) -> Result<String, BodyDescriptionDiagnostic> {
    if !is_name(&description.name) {
        return Err(BodyDescriptionDiagnostic::Encode {
            detail: "Body construction name must contain only letters, numbers, '_' or '-'".into(),
        });
    }

    let mut canonical = description.clone();
    canonical
        .hosts
        .sort_by(|left, right| left.name.cmp(&right.name));
    let mut source = String::new();
    writeln!(&mut source, "body {} {{", canonical.name).map_err(encode)?;
    writeln!(&mut source, "  schema = {}", canonical.schema).map_err(encode)?;
    writeln!(&mut source, "  id = {}", string(&canonical.body.id)?).map_err(encode)?;
    for host in &canonical.hosts {
        write!(&mut source, "  host = {{name: {}", string(&host.name)?).map_err(encode)?;
        if let Some(part) = &host.part {
            write!(&mut source, ", part: {}", string(part)?).map_err(encode)?;
        }
        write!(
            &mut source,
            ", configuration: {}, spore: {{join_mode: {}, output: {}",
            string(&host.configuration)?,
            value(&host.spore.join_mode)?,
            value(&host.spore.output)?
        )
        .map_err(encode)?;
        if let Some(invitation) = &host.spore.invitation {
            write!(&mut source, ", invitation: {}", string(invitation)?).map_err(encode)?;
        }
        source.push('}');
        if let Some(deployment) = &host.deployment {
            write!(
                &mut source,
                ", deployment: {{destination: {}}}",
                string(&deployment.destination)?
            )
            .map_err(encode)?;
        }
        source.push_str("}\n");
    }
    source.push_str("}\n");
    Ok(source)
}

fn is_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
}

fn string(value: &str) -> Result<String, BodyDescriptionDiagnostic> {
    serde_json::to_string(value).map_err(encode)
}

fn value<T: serde::Serialize>(value: &T) -> Result<String, BodyDescriptionDiagnostic> {
    serde_json::to_string(value).map_err(encode)
}

fn encode(error: impl std::fmt::Display) -> BodyDescriptionDiagnostic {
    BodyDescriptionDiagnostic::Encode {
        detail: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        parse_body_description_conduit, BodyBindingTarget, BodyHostDescription, SporeDescription,
        SporeJoinMode, BODY_DESCRIPTION_SCHEMA,
    };
    use conduit_host_fabrication::SporeOutputKind;

    #[test]
    fn canonical_source_is_sorted_and_round_trips_escaped_values() {
        let description = BodyDescription {
            schema: BODY_DESCRIPTION_SCHEMA,
            name: "new-body".into(),
            body: BodyBindingTarget {
                id: "body:new-body".into(),
            },
            hosts: vec![
                host("z-host", "part:z-host", "../z\"host.host.conduit"),
                host("a-host", "part:a-host", "../a.host.conduit"),
            ],
        };

        let source = canonical_body_description_conduit(&description).unwrap();
        assert!(source.find("a-host").unwrap() < source.find("z-host").unwrap());
        let parsed = parse_body_description_conduit(&source).unwrap();
        let mut expected = description;
        expected
            .hosts
            .sort_by(|left, right| left.name.cmp(&right.name));
        assert_eq!(parsed, expected);
    }

    #[test]
    fn invalid_construction_name_is_refused_before_source_is_emitted() {
        let description = BodyDescription {
            schema: BODY_DESCRIPTION_SCHEMA,
            name: "not a name".into(),
            body: BodyBindingTarget {
                id: "body:not-a-name".into(),
            },
            hosts: vec![host("main", "part:main", "main.host.conduit")],
        };
        assert!(matches!(
            canonical_body_description_conduit(&description),
            Err(BodyDescriptionDiagnostic::Encode { .. })
        ));
    }

    fn host(name: &str, part: &str, configuration: &str) -> BodyHostDescription {
        BodyHostDescription {
            name: name.into(),
            part: Some(part.into()),
            configuration: configuration.into(),
            spore: SporeDescription {
                join_mode: SporeJoinMode::Prejoined,
                output: SporeOutputKind::NativeBundle,
                invitation: None,
            },
            deployment: None,
        }
    }
}
