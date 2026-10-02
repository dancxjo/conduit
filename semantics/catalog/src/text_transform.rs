//! Host-neutral text catalog descriptions.

use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
use alloc::string::ToString;
#[cfg(feature = "plot-catalog")]
use alloc::vec;

pub fn text_literal_contract() -> StandardKindContract {
    describe(
        conduit_text::text_literal_semantics(),
        "Text literal",
        "Emit one bounded immutable UTF-8 startup value.",
        KindTerminalBehavior::EmitsOnce,
        "\"Hello\" >> presentation/text",
    )
}

pub fn text_upper_contract() -> StandardKindContract {
    describe(
        conduit_text::text_upper_semantics(),
        "Uppercase text",
        "Uppercase one bounded stream of UTF-8 text values.",
        KindTerminalBehavior::MirrorsInputTerminal,
        "upper: text/upper",
    )
}

pub fn text_join_contract() -> StandardKindContract {
    describe(
        conduit_text::text_join_semantics(),
        "Prefix text",
        "Prepend one immutable bounded UTF-8 prefix without an implicit separator.",
        KindTerminalBehavior::MirrorsInputTerminal,
        "join: text/join(\"Hello\")",
    )
}

#[cfg(feature = "plot-catalog")]
pub fn install_text_pipeline_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::{KindSignature, StartupParameterSignature};
    conduit_text::install_text_catalogs(startup, profile)?;
    startup.insert(KindSignature {
        kind: super::TEXT_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some(super::MAX_TEXT_VALUES.to_string()),
        }],
    })?;
    profile
        .insert_kind(
            super::text_presentation_contract()
                .into_semantic_contract(super::TEXT_PRESENTATION_CONTRACT_REVISION),
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn describe(
    contract: conduit_text::TextKindContract,
    plain_name: &str,
    summary: &str,
    terminal_behavior: KindTerminalBehavior,
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
            .map(|field| KindConfigurationField {
                key: field.key.to_string(),
                default_value: field.default_value,
                rule: KindConfigurationRule::TextBytes {
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
