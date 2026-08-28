//! Host-neutral text catalog descriptions.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::string::ToString;
#[cfg(feature = "form-catalog")]
use alloc::vec;

pub fn text_literal_contract() -> StandardKindContract {
    describe(
        conduit_text::text_literal_semantics(),
        "Text literal",
        "Emit one bounded immutable UTF-8 startup value.",
        TerminalBehavior::EmitsOnce,
        "\"Hello\" > presentation/text",
    )
}

pub fn text_upper_contract() -> StandardKindContract {
    describe(
        conduit_text::text_upper_semantics(),
        "Uppercase text",
        "Uppercase one bounded stream of UTF-8 text values.",
        TerminalBehavior::MirrorsInputTerminal,
        "upper: text/upper",
    )
}

pub fn text_join_contract() -> StandardKindContract {
    describe(
        conduit_text::text_join_semantics(),
        "Prefix text",
        "Prepend one immutable bounded UTF-8 prefix without an implicit separator.",
        TerminalBehavior::MirrorsInputTerminal,
        "join: text/join(\"Hello\")",
    )
}

#[cfg(feature = "form-catalog")]
pub fn install_text_pipeline_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    conduit_text::install_text_catalogs(startup, profile)?;
    startup.insert(KindSignature {
        kind: super::TEXT_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some(super::MAX_TEXT_VALUES.to_string()),
        }],
    })?;
    let presentation = super::text_presentation_contract();
    profile
        .insert(KindDefinition {
            kind_id: presentation.kind_id,
            kind_contract_revision: conduit_core::KindContractRevision::from(
                super::TEXT_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: presentation.inputs,
            outputs: presentation.outputs,
            configuration: vec![ConfigurationField {
                key: "maximum-values".to_string(),
                default_value: conduit_core::ConfigurationValue::U64(super::MAX_TEXT_VALUES),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: super::MAX_TEXT_VALUES,
                },
            }],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn describe(
    contract: conduit_text::TextKindContract,
    plain_name: &str,
    summary: &str,
    terminal_behavior: TerminalBehavior,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: contract.kind_id,
        plain_name: plain_name.to_string(),
        summary: summary.to_string(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract
            .configuration
            .into_iter()
            .map(|field| StandardConfigurationField {
                key: field.key.to_string(),
                default_value: field.default_value,
                rule: StandardConfigurationRule::TextBytes {
                    maximum: field.maximum_text_bytes,
                },
            })
            .collect(),
        limits: contract.limits,
        terminal_behavior,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}
