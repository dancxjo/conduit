//! Preparation-only semantic contract and exact compiled realization offer.
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const KIND: &str = "speech/english-utterance";
pub const REVISION: &str = "conduit.speech/english-utterance@1";
pub const PROFILE: &str = "conduit-native/english-s16le-8000-mono@1";
pub const IMPLEMENTATION: &str = "conduit-native/checked-english-speech@1";
pub const CAPABILITY: &str = "native-english-speech";

pub fn contract() -> Kind {
    Kind {
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        startup_parameters: vec![FrontStartupParameter {
            name: "clock".into(),
            value_type: kind_id("value/count"),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: port_id("text"),
            value_kind: kind_id("value/text"),
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
            key: "clock".into(),
            default_value: ConfigurationValue::U64(1),
            rule: KindConfigurationRule::U64Range {
                minimum: 1,
                maximum: u64::MAX,
            },
        }],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![
            FrontValueContract {
                location: FrontValueLocation::Input(port_id("text")),
                contract: CheckedValueContract::new(
                    kind_id("value/text"),
                    crate::MAXIMUM_TEXT_BYTES as u32,
                    vec![],
                )
                .expect("finite text"),
            },
            FrontValueContract {
                location: FrontValueLocation::Output(port_id("audio")),
                contract: CheckedValueContract::new(
                    kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
                    super::MAXIMUM_PCM_BYTES as u32,
                    vec![],
                )
                .expect("finite PCM"),
            },
        ])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: crate::MAXIMUM_TEXT_BYTES as u32,
        },
    }
}

/// Wrapper revision and exact checked-source identity are both artifact truth.
/// This is a compiled Host Back; it does not pretend to be an expanded Plot Back.
pub fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        contract(),
        Back {
            capability_id: CapabilityId::from(CAPABILITY),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(format!("{IMPLEMENTATION}/{}", crate::SOURCE_ID)),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub fn install(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "clock".into(),
            value_type: "Count".into(),
            default: None,
        }],
    })?;
    profile.insert_kind(contract()).map_err(|e| format!("{e}"))
}
