//! Packing changes representation only; authored acoustic semantics precede this Gear.
use alloc::{format, string::String, vec};
use conduit_audio::{
    AudioPcm16PackingInput, AUDIO_PCM_INFO_ID, PCM16_PACKING_MAXIMUM_OUTPUT_BYTES,
};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const PCM16_PACKING_KIND: &str = "audio/pack-pcm16";
pub const PCM16_PACKING_REVISION: &str = "conduit.audio/pack-pcm16@1";
pub const PCM16_PACKING_MAXIMUM_INPUT_BYTES: u32 = 32768;
pub fn pcm16_packing_semantic_contract() -> Kind {
    let ty = AudioPcm16PackingInput::semantic_type().expect("checked packing Source");
    let input = CheckedValueContract::new(
        ty.profile()
            .expect("finite packing Type")
            .value_kind()
            .clone(),
        PCM16_PACKING_MAXIMUM_INPUT_BYTES,
        vec![],
    )
    .expect("finite packing input");
    let output = CheckedValueContract::new(
        kind_id(AUDIO_PCM_INFO_ID),
        PCM16_PACKING_MAXIMUM_OUTPUT_BYTES as u32,
        vec![],
    )
    .expect("finite PCM output");
    let port = |name: &str, contract: &CheckedValueContract, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: contract.value_kind.clone(),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    let mut kind = Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(PCM16_PACKING_KIND),
        kind_contract_revision: PCM16_PACKING_REVISION.into(),
        inputs: vec![port("frames", &input, PortDirection::Input)],
        outputs: vec![port("pcm", &output, PortDirection::Output)],
        configuration: vec![KindConfigurationField {
            key: "maximum-blocks".into(),
            default_value: ConfigurationValue::U64(4),
            rule: KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: 64,
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("frames")),
                    contract: input,
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("pcm")),
                    contract: output,
                },
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            // Queue capacity is independent of the per-value envelope.
            // A parent can share the larger finite queue budget required by its DSP child.
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    };
    kind.startup_parameters = crate::startup_front(&kind.configuration);
    kind
}
pub fn install_pcm16_packing_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(conduit_audio::PCM16_PACKING_SOURCE),
        &StartupCatalog::new(),
    )
    .map_err(|e| format!("packing Source: {e:?}"))?;
    for ty in &checked.native_types {
        startup.insert_checked_native_type(&ty.name, ty)?;
    }
    startup
        .insert_value_kind_alias("PcmFrames", kind_id(AUDIO_PCM_INFO_ID))
        .map_err(|e| format!("{e:?}"))?;
    let kind = pcm16_packing_semantic_contract();
    kind.validate().map_err(|e| format!("{e:?}"))?;
    startup.insert(KindSignature {
        kind: PCM16_PACKING_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-blocks".into(),
            value_type: "Count".into(),
            default: Some("4".into()),
        }],
    })?;
    startup.insert_fore(PCM16_PACKING_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(|e| format!("{e:?}"))
}
