//! Reusable bounded text editing and line-submission semantics.

use alloc::{format, string::ToString, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
    TEXT_PRESENTATION_VALUE_KIND,
};

pub const TEXT_EDIT_KIND: &str = "text/edit";
pub const TEXT_SUBMIT_LINES_KIND: &str = "text/submit-lines";
pub const TEXT_EDIT_REVISION: &str = "conduit.text/edit@1";
pub const TEXT_SUBMIT_LINES_REVISION: &str = "conduit.text/submit-lines@1";
pub const MAXIMUM_EDITED_TEXT_BYTES: u32 = 256;

fn contract(kind: &str, revision: &str, output: &str, summary: &str) -> StandardKindContract {
    let _ = revision;
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: if kind == TEXT_EDIT_KIND {
            "Text editing"
        } else {
            "Line submission"
        }
        .to_string(),
        summary: summary.to_string(),
        inputs: vec![PortDescriptor {
            port_id: port_id("fragment"),
            value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: false },
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id(output),
            value_kind: kind_id(TEXT_PRESENTATION_VALUE_KIND),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: false },
        }],
        configuration: vec![StandardConfigurationField {
            key: "maximum-bytes".to_string(),
            default_value: ConfigurationValue::U64(256),
            rule: StandardConfigurationRule::U64Range {
                minimum: 1,
                maximum: MAXIMUM_EDITED_TEXT_BYTES as u64,
            },
        }],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_EDITED_TEXT_BYTES,
        },
        terminal_behavior: TerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: format!("state: {kind}(maximum-bytes = 256)"),
    }
}

pub fn text_edit_contract() -> StandardKindContract {
    contract(
        TEXT_EDIT_KIND,
        TEXT_EDIT_REVISION,
        "current",
        "Retain and emit bounded current text after each edit fragment.",
    )
}

pub fn text_submit_lines_contract() -> StandardKindContract {
    contract(
        TEXT_SUBMIT_LINES_KIND,
        TEXT_SUBMIT_LINES_REVISION,
        "submitted",
        "Accumulate bounded text and emit one message for each Enter fragment.",
    )
}

#[cfg(feature = "form-catalog")]
pub fn install_text_state_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for (contract, revision) in [
        (text_edit_contract(), TEXT_EDIT_REVISION),
        (text_submit_lines_contract(), TEXT_SUBMIT_LINES_REVISION),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: vec![StartupParameterSignature {
                name: "maximum-bytes".to_string(),
                value_type: "Count".to_string(),
                default: Some("256".to_string()),
            }],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: vec![ConfigurationField {
                    key: "maximum-bytes".to_string(),
                    default_value: ConfigurationValue::U64(256),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: MAXIMUM_EDITED_TEXT_BYTES as u64,
                    },
                }],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
