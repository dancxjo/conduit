//! Bounded spectral observations of immutable PCM, independent of speech intent.
use crate::{KindTerminalBehavior, StandardKindContract};
use alloc::{
    format,
    string::{String, ToString},
    vec,
};
use conduit_audio::{AudioSpectrumObservation, AUDIO_PCM_INFO_ID};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};
pub const AUDIO_SPECTRAL_ANALYSIS_KIND: &str = "audio/measure-spectrum";
pub const AUDIO_SPECTRAL_ANALYSIS_REVISION: &str = "conduit.audio/measure-spectrum@1";
pub fn audio_spectral_analysis_contract() -> StandardKindContract {
    let ty = AudioSpectrumObservation::semantic_type().expect("checked spectral observation");
    let port = |name: &str, kind: KindId, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind,
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let count = |key: &str, default, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(default),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    StandardKindContract{
        kind_id:kind_id(AUDIO_SPECTRAL_ANALYSIS_KIND),plain_name:"Measure PCM spectrum".into(),
        summary:"Emit every fitting finite FFT window with exact Frequency geometry and digital-full-scale amplitude/mean-square observations; no F0 or physical power inference.".into(),
        inputs:vec![port("pcm",kind_id(AUDIO_PCM_INFO_ID),PortDirection::Input)],
        outputs:vec![port("observation",ty.profile().expect("spectral profile").value_kind().clone(),PortDirection::Output)],
        configuration:vec![count("clock-id",1,1,u64::MAX),KindConfigurationField{key:"sample-rate".into(),default_value:ConfigurationValue::Quantity(QuantityConfigurationValue::from_value(Quantity::new(48000,Unit::Hertz)).unwrap()),rule:KindConfigurationRule::QuantityRange{minimum:8000,maximum:192000,canonical_unit:Unit::Hertz.into()}},count("source-frames",256,16,2048),count("window-frames",128,16,1024),count("stride-frames",64,1,2048),KindConfigurationField{key:"window".into(),default_value:ConfigurationValue::Text("rectangular".into()),rule:KindConfigurationRule::TextOneOf{values:vec!["rectangular".into(),"periodic-hann".into()]}},count("maximum-blocks",64,1,64)],
        limits:CapabilityLimits{max_active_instances:4,max_queue_items:1,max_queue_bytes:MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32},
        terminal_behavior:KindTerminalBehavior::CompletesWhenInputsClose,
        hosted_implementation_required:true,browser_manifestation_honest:false,pico_manifestation_honest:false,
        example:"analysis: audio/measure-spectrum(sample-rate = 48000Hz, source-frames = 256, window-frames = 128, stride-frames = 64, window = \"periodic-hann\")".into(),
    }
}
pub fn audio_spectral_analysis_semantic_contract() -> Kind {
    audio_spectral_analysis_contract().into_semantic_contract(AUDIO_SPECTRAL_ANALYSIS_REVISION)
}
pub fn install_audio_spectral_analysis_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias("PcmFrames", kind_id(AUDIO_PCM_INFO_ID))?;
    startup.insert_structured_type(
        "AudioSpectrumObservation",
        AudioSpectrumObservation::semantic_type().map_err(|e| format!("{e:?}"))?,
    )?;
    let contract = audio_spectral_analysis_semantic_contract();
    startup.insert(KindSignature {
        kind: AUDIO_SPECTRAL_ANALYSIS_KIND.into(),
        startup_parameters: contract
            .configuration
            .iter()
            .map(|f| StartupParameterSignature {
                name: f.key.clone(),
                value_type: match &f.default_value {
                    ConfigurationValue::U64(_) => "Count",
                    ConfigurationValue::Quantity(_) => "Frequency",
                    ConfigurationValue::Text(_) => "Text",
                    _ => unreachable!("spectral configuration"),
                }
                .into(),
                default: Some(match &f.default_value {
                    ConfigurationValue::U64(v) => v.to_string(),
                    ConfigurationValue::Quantity(v) => v.source().into(),
                    ConfigurationValue::Text(v) => format!("{v:?}"),
                    _ => unreachable!("spectral configuration"),
                }),
            })
            .collect(),
    })?;
    startup.insert_fore(AUDIO_SPECTRAL_ANALYSIS_KIND, contract.checked_front())?;
    profile.insert_kind(contract).map_err(|e| e.to_string())
}
pub(crate) fn audio_spectral_analysis_palette_metadata(
    kind: &KindId,
) -> Option<crate::PaletteMetadata> {
    (kind.as_str() == AUDIO_SPECTRAL_ANALYSIS_KIND).then_some(crate::PaletteMetadata {
        category: crate::PaletteCategory::Transform,
        tags: &["audio", "spectrum", "spectrogram", "fft", "measurement"],
        icon: crate::PaletteIconKey::ChartColumnsIncreasing,
    })
}
