use super::{configuration_type, sound_contracts_with_revisions};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::{ConfigurationValue, KindIdentity};
use conduit_form::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
    StartupParameterSignature,
};

/// Installs portable sound semantics and structured instrument authoring contracts.
/// This installs no Host offer: availability and implementation remain separate realization facts.
pub fn install_sound_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in sound_contracts_with_revisions() {
        install_contract(startup, profile, contract, revision)?;
    }
    crate::install_structured_music_form_catalogs(startup, profile)?;
    Ok(())
}

/// Install only the portable push-to-talk Front when another semantic owner
/// has already installed the shared `audio/play` contract.
pub fn install_audio_capture_push_to_talk_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    install_contract(
        startup,
        profile,
        super::audio_capture_push_to_talk_contract(),
        super::AUDIO_CAPTURE_PUSH_TO_TALK_REVISION,
    )
}

fn install_contract(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
    contract: super::StandardKindContract,
    revision: &'static str,
) -> Result<(), String> {
    let configuration = contract
        .configuration
        .iter()
        .map(|field| KindConfigurationField {
            key: field.key.clone(),
            default_value: field.default_value.clone(),
            rule: match &field.rule {
                KindConfigurationRule::Any => KindConfigurationRule::Any,
                KindConfigurationRule::U64Range { minimum, maximum } => {
                    KindConfigurationRule::U64Range {
                        minimum: *minimum,
                        maximum: *maximum,
                    }
                }
                KindConfigurationRule::I64Range { minimum, maximum } => {
                    KindConfigurationRule::I64Range {
                        minimum: *minimum,
                        maximum: *maximum,
                    }
                }
                KindConfigurationRule::DurationMillis { minimum, maximum } => {
                    KindConfigurationRule::DurationMillis {
                        minimum: *minimum,
                        maximum: *maximum,
                    }
                }
                KindConfigurationRule::QuantityRange {
                    minimum,
                    maximum,
                    canonical_unit,
                } => KindConfigurationRule::QuantityRange {
                    minimum: *minimum,
                    maximum: *maximum,
                    canonical_unit: *canonical_unit,
                },
                KindConfigurationRule::TextBytes { maximum } => {
                    KindConfigurationRule::TextBytes { maximum: *maximum }
                }
                KindConfigurationRule::TextOneOf { values } => KindConfigurationRule::TextOneOf {
                    values: values.clone(),
                },
                KindConfigurationRule::Structured { profile } => {
                    KindConfigurationRule::Structured {
                        profile: profile.clone(),
                    }
                }
            },
        })
        .collect::<Vec<_>>();
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().to_string(),
        startup_parameters: contract
            .configuration
            .iter()
            .map(|field| StartupParameterSignature {
                name: field.key.clone(),
                value_type: configuration_type(field).to_string(),
                default: Some(configuration_source(&field.default_value)),
            })
            .collect(),
    })?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: KindIdentity::from(revision),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration,
        })
        .map_err(|error| error.to_string())
}

fn configuration_source(value: &ConfigurationValue) -> String {
    match value {
        ConfigurationValue::Text(value) => format!("{value:?}"),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => value.to_string(),
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::Structured(value) => format!(
            "<structured:{}:{}-bytes>",
            value.profile().as_str(),
            value.canonical_value().len()
        ),
        ConfigurationValue::Quantity(value) => {
            format!("{}{}", value.value(), value.unit().form_suffix())
        }
    }
}
