//! Ordinary deterministic PCM observation, independent of speech and devices.
use crate::{KindTerminalBehavior, StandardKindContract};
use alloc::{
    format,
    string::{String, ToString},
    vec,
};
use conduit_audio::{AudioTimeAnalysisObservation, AUDIO_PCM_INFO_ID};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const AUDIO_TIME_ANALYSIS_KIND: &str = "audio/measure-time";
pub const AUDIO_TIME_ANALYSIS_REVISION: &str = "conduit.audio/measure-time@1";
pub const AUDIO_TIME_ANALYSIS_MAXIMUM_BYTES: u32 = MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
pub fn audio_time_analysis_contract() -> StandardKindContract {
    let ty = AudioTimeAnalysisObservation::semantic_type().expect("checked Audio observation");
    let port = |name: &str, kind: KindId, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind,
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let field = |key: &str, default, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(default),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    StandardKindContract {
        kind_id: kind_id(AUDIO_TIME_ANALYSIS_KIND), plain_name: "Measure PCM time window".into(),
        summary: "Observe an exact mono PCM interval with digital-full-scale waveform, mean-square, RMS and peak; no acoustic inference.".into(),
        inputs: vec![port("pcm", kind_id(AUDIO_PCM_INFO_ID), PortDirection::Input)],
        outputs: vec![port("observation", ty.profile().expect("finite Audio Type").value_kind().clone(), PortDirection::Output)],
        configuration: vec![field("clock-id", 1, 1, u64::MAX), field("sample-rate-hz", 8000, 8000, 192000),
            field("offset", 0, 0, 2046), field("frames", 4, 2, 2048), field("maximum-blocks", 64, 1, 64)],
        limits: CapabilityLimits { max_active_instances: 4, max_queue_items: 1, max_queue_bytes: AUDIO_TIME_ANALYSIS_MAXIMUM_BYTES },
        terminal_behavior: KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required: true, browser_manifestation_honest: false, pico_manifestation_honest: false,
        example: "analysis: audio/measure-time(clock-id = 1, sample-rate-hz = 8000, frames = 4)".into(),
    }
}
pub fn audio_time_analysis_semantic_contract() -> Kind {
    audio_time_analysis_contract().into_semantic_contract(AUDIO_TIME_ANALYSIS_REVISION)
}

pub fn install_audio_time_analysis_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup
        .insert_value_kind_alias("PcmFrames", kind_id(AUDIO_PCM_INFO_ID))
        .map_err(|e| e.to_string())?;
    startup.insert_structured_type(
        "AudioTimeAnalysisObservation",
        AudioTimeAnalysisObservation::semantic_type().map_err(|e| format!("{e:?}"))?,
    )?;
    let contract = audio_time_analysis_semantic_contract();
    startup.insert(KindSignature {
        kind: AUDIO_TIME_ANALYSIS_KIND.into(),
        startup_parameters: contract
            .configuration
            .iter()
            .map(|f| StartupParameterSignature {
                name: f.key.clone(),
                value_type: "Count".into(),
                default: Some(match f.default_value {
                    ConfigurationValue::U64(value) => value.to_string(),
                    _ => unreachable!("count configuration"),
                }),
            })
            .collect(),
    })?;
    startup.insert_fore(AUDIO_TIME_ANALYSIS_KIND, contract.checked_front())?;
    profile.insert_kind(contract).map_err(|e| e.to_string())
}

/// Discovery decoration stays outside the Kind's semantic identity.
pub(crate) fn audio_time_analysis_palette_metadata(
    kind: &KindId,
) -> Option<crate::PaletteMetadata> {
    (kind.as_str() == AUDIO_TIME_ANALYSIS_KIND).then_some(crate::PaletteMetadata {
        category: crate::PaletteCategory::Transform,
        tags: &["audio", "pcm", "waveform", "rms", "energy", "measurement"],
        icon: crate::PaletteIconKey::ChartColumnsIncreasing,
    })
}
