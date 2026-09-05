//! Portable command and result contracts for bounded named-pattern storage.

use alloc::{
    string::{String, ToString},
    vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue, StructuredVariantCase,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

pub const TEMPLATE_STORAGE_KIND: &str = "storage/named-pattern-templates";
pub const TEMPLATE_STORAGE_REVISION: &str = "conduit.std/named-pattern-templates@1";
pub const TEMPLATE_STORAGE_COMMAND_TYPE: &str = "NamedPatternTemplateCommand";
pub const TEMPLATE_STORAGE_RESULT_TYPE: &str = "NamedPatternTemplateResult";
pub const MAXIMUM_TEMPLATE_STORAGE_COMMANDS: u64 = 16;

pub fn named_pattern_template_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("sequence/named-pattern-template@1"),
        vec![
            StructuredFieldType::new(
                "name",
                StructuredInfoType::leaf(kind_id(crate::TEMPLATE_NAME_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new("pattern", crate::normalized_duration_sequence_type())
                .unwrap(),
        ],
    )
    .unwrap()
}

pub fn template_storage_command_type() -> StructuredInfoType {
    let name = StructuredInfoType::leaf(kind_id(crate::TEMPLATE_NAME_INFO_ID)).unwrap();
    StructuredInfoType::variant(
        kind_id("storage/named-pattern-template-command@1"),
        vec![
            StructuredVariantCase::new("delete", name.clone()).unwrap(),
            StructuredVariantCase::new("get", name).unwrap(),
            StructuredVariantCase::new("put", named_pattern_template_type()).unwrap(),
        ],
    )
    .unwrap()
}

pub fn template_storage_result_type() -> StructuredInfoType {
    let name = StructuredInfoType::leaf(kind_id(crate::TEMPLATE_NAME_INFO_ID)).unwrap();
    StructuredInfoType::variant(
        kind_id("storage/named-pattern-template-result@1"),
        vec![
            StructuredVariantCase::new("deleted", name.clone()).unwrap(),
            StructuredVariantCase::new("found", named_pattern_template_type()).unwrap(),
            StructuredVariantCase::new("missing", name.clone()).unwrap(),
            StructuredVariantCase::new("stored", name).unwrap(),
        ],
    )
    .unwrap()
}

pub fn named_pattern_template_storage_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TEMPLATE_STORAGE_KIND),
        kind_contract_revision: KindContractRevision::from(TEMPLATE_STORAGE_REVISION),
        inputs: vec![port(
            "command",
            &template_storage_command_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "result",
            &template_storage_result_type(),
            PortDirection::Output,
        )],
        configuration: vec![ConfigurationField {
            key: "maximum-commands".into(),
            default_value: ConfigurationValue::U64(MAXIMUM_TEMPLATE_STORAGE_COMMANDS),
            validation: ConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAXIMUM_TEMPLATE_STORAGE_COMMANDS,
            },
        }],
    }
}

pub fn install_template_storage_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_structured_type(
            TEMPLATE_STORAGE_COMMAND_TYPE,
            template_storage_command_type(),
        )
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(TEMPLATE_STORAGE_RESULT_TYPE, template_storage_result_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: TEMPLATE_STORAGE_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "maximum-commands".into(),
                value_type: "Count".into(),
                default: Some(MAXIMUM_TEMPLATE_STORAGE_COMMANDS.to_string()),
            }],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(named_pattern_template_storage_definition())
        .map_err(|error| error.to_string())
}

pub fn put_template_command(
    name: &str,
    pattern: StructuredInfoValue,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    let template = named_template(name, pattern)?;
    StructuredInfoValue::variant(template_storage_command_type(), "put", template)
        .map_err(|_| crate::TemplateCollectionRefusal::Malformed)
}

pub fn get_template_command(
    name: &str,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    name_variant(template_storage_command_type(), "get", name)
}

pub fn delete_template_command(
    name: &str,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    name_variant(template_storage_command_type(), "delete", name)
}

pub fn stored_template_result(
    name: &str,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    name_variant(template_storage_result_type(), "stored", name)
}

pub fn missing_template_result(
    name: &str,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    name_variant(template_storage_result_type(), "missing", name)
}

pub fn found_template_result(
    name: &str,
    pattern: StructuredInfoValue,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    StructuredInfoValue::variant(
        template_storage_result_type(),
        "found",
        named_template(name, pattern)?,
    )
    .map_err(|_| crate::TemplateCollectionRefusal::Malformed)
}

fn named_template(
    name: &str,
    pattern: StructuredInfoValue,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    validate_name(name)?;
    crate::compare_normalized_patterns(&pattern, &pattern, crate::MAXIMUM_ABSOLUTE_METRIC, 0)
        .map_err(|_| crate::TemplateCollectionRefusal::CorruptTemplate)?;
    StructuredInfoValue::record(
        named_pattern_template_type(),
        vec![
            StructuredFieldValue::new("name", name_value(name)?)
                .map_err(|_| crate::TemplateCollectionRefusal::Malformed)?,
            StructuredFieldValue::new("pattern", pattern)
                .map_err(|_| crate::TemplateCollectionRefusal::Malformed)?,
        ],
    )
    .map_err(|_| crate::TemplateCollectionRefusal::Malformed)
}

fn name_variant(
    value_type: StructuredInfoType,
    tag: &str,
    name: &str,
) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    StructuredInfoValue::variant(value_type, tag, name_value(name)?)
        .map_err(|_| crate::TemplateCollectionRefusal::Malformed)
}

fn name_value(name: &str) -> Result<StructuredInfoValue, crate::TemplateCollectionRefusal> {
    validate_name(name)?;
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(crate::TEMPLATE_NAME_INFO_ID)).unwrap(),
        name.as_bytes().to_vec(),
    )
    .map_err(|_| crate::TemplateCollectionRefusal::Malformed)
}

fn validate_name(name: &str) -> Result<(), crate::TemplateCollectionRefusal> {
    if name.is_empty() {
        return Err(crate::TemplateCollectionRefusal::NameEmpty);
    }
    if name.len() > crate::MAXIMUM_TEMPLATE_NAME_BYTES {
        return Err(crate::TemplateCollectionRefusal::NameTooLong);
    }
    Ok(())
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_results_are_typed_bounded_and_domain_neutral() {
        let pattern = crate::normalized_value(&[500_000, 1_000_000]).unwrap();
        for value in [
            put_template_command("cadence", pattern.clone()).unwrap(),
            get_template_command("cadence").unwrap(),
            delete_template_command("cadence").unwrap(),
        ] {
            assert_eq!(value.value_type(), &template_storage_command_type());
        }
        assert_eq!(
            found_template_result("cadence", pattern)
                .unwrap()
                .value_type(),
            &template_storage_result_type()
        );
        let definition = named_pattern_template_storage_definition();
        assert_eq!(
            definition.inputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert_eq!(
            definition.outputs[0].temporal,
            PortTemporal::Flow { closes: true }
        );
        assert!(!alloc::format!("{definition:?}").contains("secret"));
    }
}
