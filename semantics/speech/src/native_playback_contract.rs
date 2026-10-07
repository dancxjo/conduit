//! Exact data-bound prepared-tape offer, installed before source checking.
//! This profile is one immutable native tape, with no text frontend or live
//! mutation of a checked/expanded/realized identity.
#[path = "native_playback_literal.rs"]
mod literal;
use crate::{playback_basis::PreparedSpeechPlaybackTape, semantic::SpeechPlaybackBasis, SOURCE_ID};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

pub const KIND: &str = "speech/render-admitted";
pub const REVISION: &str = "conduit.speech/render-admitted@1";
pub const PROFILE: &str = "conduit-native/admitted-s16le-8000-mono@1";
pub const IMPLEMENTATION: &str = "conduit-native/admitted-speech-tape@1";
pub const REQUIRED_STEP_FUEL: u16 = 3;
pub const MAXIMUM_PCM_BYTES: usize =
    conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN + 2 * crate::MAXIMUM_BLOCK_FRAMES;

pub fn basis_configuration(basis: &SpeechPlaybackBasis) -> Result<ConfigurationValue, String> {
    let profile = SpeechPlaybackBasis::semantic_type()
        .map_err(|error| format!("{error:?}"))?
        .profile()
        .map_err(|error| format!("{error:?}"))?
        .value_kind()
        .clone();
    StructuredConfigurationValue::new(
        profile,
        basis
            .clone()
            .encode()
            .map_err(|error| format!("{error:?}"))?,
    )
    .map(ConfigurationValue::Structured)
    .ok_or_else(|| "finite playback basis".into())
}
pub fn contract(tape: &PreparedSpeechPlaybackTape<'_>) -> Result<Kind, String> {
    let basis = basis_configuration(tape.basis())?;
    let ConfigurationValue::Structured(value) = &basis else {
        unreachable!()
    };
    Ok(Kind {
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        startup_parameters: vec![FrontStartupParameter {
            name: "basis".into(),
            value_type: value.profile().clone(),
            has_default: true,
        }],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("start"),
            value_kind: kind_id("value/count"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("audio"),
            value_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "basis".into(),
            rule: KindConfigurationRule::Structured {
                profile: value.profile().clone(),
            },
            default_value: basis,
        }],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![
            FrontValueContract {
                location: FrontValueLocation::Input(port_id("start")),
                contract: CheckedValueContract::new(kind_id("value/count"), 8, vec![])
                    .map_err(|error| format!("{error:?}"))?,
            },
            FrontValueContract {
                location: FrontValueLocation::Output(port_id("audio")),
                contract: CheckedValueContract::new(
                    kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
                    MAXIMUM_PCM_BYTES as u32,
                    vec![],
                )
                .map_err(|error| format!("{error:?}"))?,
            },
        ])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_PCM_BYTES as u32,
        },
    })
}
pub fn offer(tape: &PreparedSpeechPlaybackTape<'_>) -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        contract(tape)?,
        Back {
            capability_id: CapabilityId::from("admitted-speech-tape"),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(format!("{IMPLEMENTATION}/{SOURCE_ID}")),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn install(
    tape: &PreparedSpeechPlaybackTape<'_>,
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.ensure_structured_type(
        "SpeechPlaybackBasis",
        SpeechPlaybackBasis::semantic_type().map_err(|error| format!("{error:?}"))?,
    )?;
    startup
        .insert(KindSignature {
            kind: KIND.into(),
            startup_parameters: vec![conduit_plot::StartupParameterSignature {
                name: "basis".into(),
                value_type: "SpeechPlaybackBasis".into(),
                default: Some(literal::literal(
                    &tape
                        .basis()
                        .clone()
                        .into_structured()
                        .map_err(|error| format!("{error:?}"))?,
                )?),
            }],
        })
        .map_err(|error| format!("{error:?}"))?;
    let literal = literal::literal(
        &tape
            .basis()
            .clone()
            .into_structured()
            .map_err(|error| format!("{error:?}"))?,
    )?;
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&format!(
            "plot playback-basis (\n basis: SpeechPlaybackBasis = {literal}\n) {{\n}}\n"
        )),
        startup,
    )
    .map_err(|error| format!("{error:?}"))?;
    let Some(conduit_plot::CanonicalStartupValue::Structured(value)) =
        &checked.plots[0].startup_parameters[0].default
    else {
        return Err("native basis literal".into());
    };
    if value
        .try_concrete()
        .and_then(|value| value.canonical_bytes().ok())
        != Some(
            tape.basis()
                .clone()
                .encode()
                .map_err(|error| format!("{error:?}"))?,
        )
    {
        return Err("native basis literal roundtrip".into());
    }
    profile
        .insert_kind(contract(tape)?)
        .map_err(|error| format!("{error:?}"))
}
