use super::{configuration_type, sound_contracts_with_revisions};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use conduit_core::ConfigurationValue;
use conduit_plot::{KindSignature, StartupParameterSignature};

/// Installs portable sound semantics and structured instrument authoring contracts.
/// This installs no Host offer: availability and implementation remain separate realization facts.
pub fn install_sound_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (contract, revision) in sound_contracts_with_revisions() {
        if contract.kind_id.as_str() == super::AUDIO_TONE_KIND {
            install_semantic_contract(startup, profile, super::audio_tone_semantic_contract())?;
        } else if contract.kind_id.as_str() == super::AUDIO_CONTINUOUS_TONE_KIND {
            install_semantic_contract(
                startup,
                profile,
                super::audio_continuous_tone_semantic_contract(),
            )?;
        } else {
            install_contract(startup, profile, contract, revision)?;
        }
    }
    crate::install_structured_music_plot_catalogs(startup, profile)?;
    Ok(())
}

/// Install the canonical playback signature and exact configuration together.
pub fn install_audio_play_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_contract(
        startup,
        profile,
        super::audio_play_contract(),
        super::AUDIO_PLAY_REVISION,
    )
}

/// Install only the portable `audio/tone` transform contract.
///
/// Hosts that cannot realize tone synthesis still need this semantic contract
/// to check reviewed Plots before realization eligibility is considered.
pub fn install_audio_tone_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_semantic_contract(startup, profile, super::audio_tone_semantic_contract())
}

/// Install only the portable sustained-tone transform contract.
pub fn install_audio_continuous_tone_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_semantic_contract(
        startup,
        profile,
        super::audio_continuous_tone_semantic_contract(),
    )
}

/// Install only the portable `audio/apply-gain` transform contract.
pub fn install_audio_gain_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_contract(
        startup,
        profile,
        super::audio_gain_contract(),
        super::AUDIO_GAIN_REVISION,
    )
}

fn install_semantic_contract(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    contract: conduit_core::Kind,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().to_string(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(contract)
        .map_err(|error| error.to_string())
}

/// Install only the portable push-to-talk Front when another semantic owner
/// has already installed the shared `audio/play` contract.
pub fn install_audio_capture_push_to_talk_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    install_contract(
        startup,
        profile,
        super::audio_capture_push_to_talk_contract(),
        super::AUDIO_CAPTURE_PUSH_TO_TALK_REVISION,
    )
}

fn install_contract(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
    contract: super::StandardKindContract,
    revision: &'static str,
) -> Result<(), String> {
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
        .insert_kind(contract.into_semantic_contract(revision))
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
            format!("{}{}", value.value(), value.unit().plot_suffix())
        }
    }
}
