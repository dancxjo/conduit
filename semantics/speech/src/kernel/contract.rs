//! Preparation-only semantic contract and exact compiled realization offer.
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const KIND: &str = "speech/utterance";
pub const REVISION: &str = "conduit.speech/utterance@2";
pub const PROFILE: &str = "conduit-native/english-s16le-8000-mono@1";
pub const IMPLEMENTATION: &str = "conduit-native/checked-english-speech@3";
pub const CAPABILITY: &str = "native-english-speech";

pub fn contract() -> Kind {
    let mut kind = Kind {
        kind_id: kind_id(KIND),
        kind_contract_revision: KindIdentity::from(REVISION),
        startup_parameters: vec![
            FrontStartupParameter {
                name: "clock".into(),
                value_type: kind_id("value/count"),
                has_default: false,
            },
            conduit_language::language_request_parameter(),
        ],
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
        configuration: vec![
            KindConfigurationField {
                key: "clock".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: u64::MAX,
                },
            },
            conduit_language::language_request_field(),
        ],
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
    };
    kind.semantic_laws
        .extend(conduit_language::language_requirement_laws());
    kind
}

/// Wrapper revision and exact checked-source identity are both artifact truth.
/// This is a compiled Host Back; it does not pretend to be an expanded Plot Back.
pub fn offer() -> CapabilityOffer {
    let mut offered = BackOfferBuilder::new(
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
    .build();
    offered.realization_properties =
        vec![
            conduit_language::language_coverage_property(english_coverage())
                .expect("compiled source coverage"),
        ];
    offered
}

pub fn install(startup: &mut StartupCatalog, profile: &mut ProfileCatalog) -> Result<(), String> {
    conduit_language::install_language_request_type(startup)?;
    startup.insert_value_kind_alias("PcmFrames", kind_id(conduit_audio::AUDIO_PCM_INFO_ID))?;
    startup.insert(KindSignature {
        kind: KIND.into(),
        startup_parameters: vec![
            StartupParameterSignature {
                name: "clock".into(),
                value_type: "Count".into(),
                default: None,
            },
            conduit_language::language_request_signature(),
        ],
    })?;
    profile.insert_kind(contract()).map_err(|e| format!("{e}"))
}

fn english_coverage() -> conduit_language::LanguageCoverage {
    use conduit_language::{LanguageCoverage, LanguageId, LanguageVariety, VarietyId};
    use conduit_plot::rust_binding::BoundedSequence;
    let language = LanguageId::new("language/english".into()).expect("finite identity");
    let variety = LanguageVariety::new(
        VarietyId::new("pronunciation/native-english@2".into()).expect("finite identity"),
        language.clone(),
    )
    .expect("exact relationship");
    LanguageCoverage::new(
        "repository/native-english-checked-source".into(),
        BoundedSequence::try_from_iter([language]).expect("one language"),
        BoundedSequence::try_from_iter([]).expect("no private mapping"),
        "native-english@2".into(),
        BoundedSequence::try_from_iter([variety]).expect("one pronunciation profile"),
        true,
    )
    .expect("compiled source declaration")
}
