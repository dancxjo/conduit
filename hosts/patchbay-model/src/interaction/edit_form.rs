//! Form-facing contract and exact reconstruction for typed authoring edits.

use super::*;

pub(super) fn edit_signature() -> KindSignature {
    KindSignature {
        kind: EDIT_KIND.into(),
        startup_parameters: edit_configuration()
            .into_iter()
            .map(|field| StartupParameterSignature {
                name: field.key,
                value_type: match field.default_value {
                    ConfigurationValue::Bool(_) => "Boolean",
                    ConfigurationValue::U64(_) => "Count",
                    ConfigurationValue::I64(_) => "Scalar",
                    ConfigurationValue::Text(_) => "Text",
                }
                .into(),
                default: None,
            })
            .collect(),
    }
}

pub(super) fn edit_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(EDIT_KIND),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        inputs: vec![],
        outputs: vec![request_port(PortDirection::Output)],
        configuration: edit_configuration(),
    }
}

fn edit_configuration() -> Vec<ConfigurationField> {
    vec![
        text_field("request"),
        text_field("source"),
        ConfigurationField {
            key: "revision".into(),
            default_value: ConfigurationValue::U64(0),
            validation: ConfigurationRule::U64Range {
                minimum: 0,
                maximum: u64::MAX,
            },
        },
        text_field("basis"),
        ConfigurationField {
            key: "operation".into(),
            default_value: ConfigurationValue::Text(String::new()),
            validation: ConfigurationRule::TextOneOf {
                values: vec![
                    "place-gear".into(),
                    "duplicate-gear".into(),
                    "remove-gear".into(),
                    "remove-cord".into(),
                    "connect-ports".into(),
                    "reroute-cord".into(),
                    "configure-gear".into(),
                ],
            },
        },
        text_field("primary"),
        text_field("secondary"),
        text_field("key"),
        ConfigurationField {
            key: "value-type".into(),
            default_value: ConfigurationValue::Text(String::new()),
            validation: ConfigurationRule::TextOneOf {
                values: vec![
                    "none".into(),
                    "bool".into(),
                    "count".into(),
                    "scalar".into(),
                    "text".into(),
                ],
            },
        },
        ConfigurationField {
            key: "bool-value".into(),
            default_value: ConfigurationValue::Bool(false),
            validation: ConfigurationRule::Any,
        },
        ConfigurationField {
            key: "count-value".into(),
            default_value: ConfigurationValue::U64(0),
            validation: ConfigurationRule::U64Range {
                minimum: 0,
                maximum: u64::MAX,
            },
        },
        ConfigurationField {
            key: "scalar-value".into(),
            default_value: ConfigurationValue::I64(0),
            validation: ConfigurationRule::I64Range {
                minimum: i64::MIN,
                maximum: i64::MAX,
            },
        },
        text_field("text-value"),
    ]
}

fn text_field(key: &str) -> ConfigurationField {
    ConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(String::new()),
        validation: ConfigurationRule::TextBytes {
            maximum: MAX_INTERACTION_ID_BYTES as u32,
        },
    }
}

pub(super) fn edit_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: edit_signature()
            .startup_parameters
            .into_iter()
            .map(|parameter| FaceStartupParameter {
                name: parameter.name,
                value_type: parameter.value_type,
                has_default: false,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from("patchbay-edit"),
        kind_id: kind_id(EDIT_KIND),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
            implementation_id: ImplementationId::from("patchbay/edit@1"),
            artifact_id: ArtifactId::from("patchbay/edit@1"),
        },
        inputs: vec![],
        outputs: vec![request_port(PortDirection::Output)],
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: interaction_limits(),
    }
}

pub(super) fn request_source(request: &PatchbayInteractionRequest) -> String {
    let (kind, fields) = match request {
        PatchbayInteractionRequest::Select {
            request_id,
            expanded_form_id,
            subject_identity,
        } => (
            SELECT_KIND,
            vec![
                request_id.as_str(),
                expanded_form_id.as_str(),
                subject_identity.as_str(),
            ],
        ),
        PatchbayInteractionRequest::Invoke {
            request_id,
            invocation,
        } => (
            INVOKE_KIND,
            vec![
                request_id.as_str(),
                invocation.action.as_str(),
                invocation.target_identity.as_str(),
            ],
        ),
        PatchbayInteractionRequest::Edit { request_id, edit } => {
            return edit_request_source(request_id, edit)
        }
    };
    let arguments = fields
        .into_iter()
        .map(|field| format!("\"{}\"", escape_form_text(field)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "form patchbay-interaction {{\n    request: {kind}({arguments})\n    apply: {APPLY_KIND}\n    request > apply\n}}\n"
    )
}

fn edit_request_source(request_id: &PatchbayInteractionRequestId, edit: &PatchbayEdit) -> String {
    let basis = edit.basis();
    let (primary, secondary, key, value) = match edit {
        PatchbayEdit::PlaceGear { kind_id, .. } => (kind_id.as_str(), "", "", None),
        PatchbayEdit::DuplicateGear {
            subject_identity, ..
        }
        | PatchbayEdit::RemoveGear {
            subject_identity, ..
        }
        | PatchbayEdit::RemoveCord {
            subject_identity, ..
        } => (subject_identity.as_str(), "", "", None),
        PatchbayEdit::ConnectPorts {
            source_identity,
            sink_identity,
            ..
        } => (source_identity.as_str(), sink_identity.as_str(), "", None),
        PatchbayEdit::RerouteCord {
            cord_identity,
            endpoint_identity,
            ..
        } => (cord_identity.as_str(), endpoint_identity.as_str(), "", None),
        PatchbayEdit::ConfigureGear {
            subject_identity,
            key,
            value,
            ..
        } => (subject_identity.as_str(), "", key.as_str(), Some(value)),
    };
    let (value_type, bool_value, count_value, scalar_value, text_value) = match value {
        None => ("none", false, 0, 0, ""),
        Some(ConfigurationValue::Bool(value)) => ("bool", *value, 0, 0, ""),
        Some(ConfigurationValue::U64(value)) => ("count", false, *value, 0, ""),
        Some(ConfigurationValue::I64(value)) => ("scalar", false, 0, *value, ""),
        Some(ConfigurationValue::Text(value)) => ("text", false, 0, 0, value.as_str()),
    };
    format!(
        "form patchbay-interaction {{\n    request: {EDIT_KIND}(\"{}\", \"{}\", {}, \"{}\", \"{}\", \"{}\", \"{}\", \"{}\", \"{}\", {}, {}, {}, \"{}\")\n    apply: {APPLY_KIND}\n    request > apply\n}}\n",
        escape_form_text(request_id.as_str()),
        escape_form_text(basis.source_document_id.as_str()),
        basis.source_revision,
        escape_form_text(basis.expanded_form_id.as_str()),
        edit.operation(),
        escape_form_text(primary),
        escape_form_text(secondary),
        escape_form_text(key),
        value_type,
        bool_value,
        count_value,
        scalar_value,
        escape_form_text(text_value),
    )
}

pub(super) fn edit_from_configuration(
    configuration: &[conduit_core::ConfigurationEntry],
) -> Result<PatchbayEdit, InteractionError> {
    let value = |key: &str| {
        configuration
            .iter()
            .find(|entry| entry.key.as_str() == key)
            .map(|entry| &entry.value)
            .ok_or_else(|| InteractionError::Form(format!("interaction field '{key}' is absent")))
    };
    let text = |key: &str| match value(key)? {
        ConfigurationValue::Text(value) => Ok(value.clone()),
        _ => Err(InteractionError::MalformedValue),
    };
    let revision = match value("revision")? {
        ConfigurationValue::U64(value) => *value,
        _ => return Err(InteractionError::MalformedValue),
    };
    let basis = PatchbayEditBasis::new(
        SourceDocumentId::from(text("source")?),
        revision,
        ExpandedFormId::from(text("basis")?),
    )?;
    let primary = text("primary")?;
    let secondary = text("secondary")?;
    let edit = match text("operation")?.as_str() {
        "place-gear" => PatchbayEdit::PlaceGear {
            basis,
            kind_id: primary,
        },
        "duplicate-gear" => PatchbayEdit::DuplicateGear {
            basis,
            subject_identity: primary,
        },
        "remove-gear" => PatchbayEdit::RemoveGear {
            basis,
            subject_identity: primary,
        },
        "remove-cord" => PatchbayEdit::RemoveCord {
            basis,
            subject_identity: primary,
        },
        "connect-ports" => PatchbayEdit::ConnectPorts {
            basis,
            source_identity: primary,
            sink_identity: secondary,
        },
        "reroute-cord" => PatchbayEdit::RerouteCord {
            basis,
            cord_identity: primary,
            endpoint_identity: secondary,
        },
        "configure-gear" => {
            let configured = match text("value-type")?.as_str() {
                "bool" => value("bool-value")?.clone(),
                "count" => value("count-value")?.clone(),
                "scalar" => value("scalar-value")?.clone(),
                "text" => value("text-value")?.clone(),
                _ => return Err(InteractionError::MalformedValue),
            };
            PatchbayEdit::ConfigureGear {
                basis,
                subject_identity: primary,
                key: text("key")?,
                value: configured,
            }
        }
        _ => return Err(InteractionError::MalformedValue),
    };
    edit.validate()?;
    Ok(edit)
}

fn escape_form_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
