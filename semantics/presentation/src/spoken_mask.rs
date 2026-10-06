//! Exact evidence at the terminal boundary of an artifact-output spoken Mask.
//!
//! Writing an audio artifact is a truthful output effect. It is neither a
//! claim that a speaker played it nor evidence that a human heard it.

use alloc::string::String;
use conduit_core::{ActivePlayId, PlacementId, PlanId, SignId};
use serde::{Deserialize, Serialize};

use crate::{
    GeneratedContentRole, GeneratedManifestation, ManifestationLifecycle, MaskShow, MaskShowError,
    Presentation, MAX_GENERATED_WORDING_BYTES,
};

pub const SPOKEN_MASK_ARTIFACT_RECEIPT_KIND: &str =
    "conduit.presentation/spoken-mask-artifact-receipt@1";
pub const MAX_SPOKEN_MASK_ARTIFACT_IDENTITY_BYTES: usize = 256;
pub const PRESENTATION_TO_GENERATIVE_REQUEST_KIND: &str = "presentation/adapt-generative-request";
pub const GENERATED_VALIDATION_ENVELOPE_KIND: &str =
    "presentation/build-generated-validation-envelope";
pub const RETAIN_GENERATED_VALIDATION_KIND: &str = "presentation/retain-generated-validation";
pub const GENERATED_MANIFESTATION_TO_SPEECH_KIND: &str =
    "presentation/generated-manifestation-speech";
pub const SPOKEN_ARTIFACT_KIND: &str = "presentation/spoken-artifact";
pub const GENERATED_MANIFESTATION_TO_SPEECH_STREAM_KIND: &str =
    "presentation/generated-manifestation-speech-stream";
pub const ARTIFACT_ACKNOWLEDGED_SHOW_KIND: &str = "presentation/artifact-acknowledged-show";
pub const CLOSING_NO_INTERACTION_KIND: &str = "presentation/no-interaction";
pub const DIRECT_FACE_WORDING_KIND: &str = "presentation/direct-face-wording";
pub const DIRECT_ARTIFACT_SHOW_KIND: &str = "presentation/direct-artifact-show";
pub const SPOKEN_MASK_CONTRACT_REVISION: &str = "conduit.presentation/spoken-mask-stage@1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskArtifactReceipt {
    pub artifact_identity: String,
    pub content_sha256: String,
    /// Host-retained local file, when this Back can expose one. This is an
    /// evidence locator, never an authority or an authored Plot address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_locator: Option<String>,
    pub pcm_bytes: u32,
    pub frames: u32,
    pub blocks: u16,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub placement_id: PlacementId,
    pub completion_sign_id: SignId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAcknowledgedSpokenShow {
    pub show: MaskShow,
    pub generated_manifestation_identity: String,
    /// Exact validated outward Speech handed to synthesis for this Show.
    pub accepted_wording: String,
    /// Retained from the same validated candidate that produced this Show.
    pub generation_evidence: SpokenGenerationEvidence,
    pub artifact: SpokenMaskArtifactReceipt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenGenerationEvidence {
    pub provider_identity: String,
    pub model_identity: String,
    pub candidate_digest: String,
    pub validation_receipt_identity: String,
    pub original_model_output: Option<String>,
}

/// Direct Face wording needs its own artifact witness. It has no generated
/// manifestation or model claim; the Show still belongs to the exact Mask
/// Plan and becomes Available only after the audio artifact is acknowledged.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirectArtifactAcknowledgedSpokenShow {
    pub show: MaskShow,
    pub artifact: SpokenMaskArtifactReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpokenMaskShowError {
    InvalidArtifactIdentity,
    EmptyArtifact,
    NotAvailable,
    StalePresentation,
    StaleGeneration,
    StalePlan,
    InvalidShow(MaskShowError),
}

impl ArtifactAcknowledgedSpokenShow {
    /// Outer-owner check after the trusted spoken Mask session has already
    /// validated the generated candidate and minted this Show. This proves
    /// artifact/Face/Plan/Play linkage, not the candidate's semantics by itself.
    pub fn validate_owner_artifact(
        &self,
        presentation: &Presentation,
    ) -> Result<(), SpokenMaskShowError> {
        self.artifact.validate_for(&self.show)?;
        if self.generated_manifestation_identity.is_empty()
            || self.accepted_wording.is_empty()
            || self.accepted_wording.len() > MAX_GENERATED_WORDING_BYTES
            || self.generation_evidence.provider_identity.is_empty()
            || self.generation_evidence.model_identity.is_empty()
            || self.generation_evidence.candidate_digest.is_empty()
            || self
                .generation_evidence
                .validation_receipt_identity
                .is_empty()
        {
            return Err(SpokenMaskShowError::StaleGeneration);
        }
        if self.show.show.lifecycle != ManifestationLifecycle::Available {
            return Err(SpokenMaskShowError::NotAvailable);
        }
        self.show
            .validate(presentation)
            .map_err(SpokenMaskShowError::InvalidShow)
    }

    pub fn validate(
        &self,
        presentation: &Presentation,
        generated: &GeneratedManifestation,
    ) -> Result<(), SpokenMaskShowError> {
        self.validate_owner_artifact(presentation)?;
        if self.generated_manifestation_identity != generated.manifestation_identity()
            || self.generation_evidence.provider_identity != generated.candidate().provider_identity
            || self.generation_evidence.model_identity != generated.candidate().model_identity
            || self.generation_evidence.candidate_digest
                != generated.validation_receipt().candidate_digest()
            || self.generation_evidence.validation_receipt_identity
                != generated.validation_receipt().receipt_identity()
            || self.generation_evidence.original_model_output.as_deref()
                != generated.validation_receipt().raw_provider_output()
            || generated.source_presentation_identity() != presentation.identity.as_str()
            || generated.source_presentation_revision() != presentation.revision
            || generated
                .content()
                .iter()
                .filter(|segment| segment.role == GeneratedContentRole::Speech)
                .count()
                != 1
            || !generated.content().iter().any(|segment| {
                segment.role == GeneratedContentRole::Speech
                    && segment.bytes == self.accepted_wording.as_bytes()
            })
        {
            return Err(SpokenMaskShowError::StaleGeneration);
        }
        Ok(())
    }
}

impl DirectArtifactAcknowledgedSpokenShow {
    pub fn validate(&self, presentation: &Presentation) -> Result<(), SpokenMaskShowError> {
        self.artifact.validate_for(&self.show)?;
        if self.show.show.lifecycle != ManifestationLifecycle::Available {
            return Err(SpokenMaskShowError::NotAvailable);
        }
        self.show
            .validate(presentation)
            .map_err(SpokenMaskShowError::InvalidShow)
    }
}

impl SpokenMaskArtifactReceipt {
    fn validate_for(&self, show: &MaskShow) -> Result<(), SpokenMaskShowError> {
        if self.artifact_identity.is_empty()
            || self.artifact_identity.len() > MAX_SPOKEN_MASK_ARTIFACT_IDENTITY_BYTES
            || self
                .artifact_locator
                .as_ref()
                .is_some_and(|locator| locator.is_empty() || locator.len() > 4096)
            || self.content_sha256.len() != 64
            || !self
                .content_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(SpokenMaskShowError::InvalidArtifactIdentity);
        }
        if self.pcm_bytes == 0 || self.frames == 0 || self.blocks == 0 {
            return Err(SpokenMaskShowError::EmptyArtifact);
        }
        if self.plan_id != show.planned_mask.plan.plan_id
            || self.active_play_id != show.show.active_play_id
            || !show
                .planned_mask
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .any(|placement| {
                    placement.placement_id == self.placement_id
                        && placement.kind_id.as_str() == SPOKEN_ARTIFACT_KIND
                })
        {
            return Err(SpokenMaskShowError::StalePlan);
        }
        Ok(())
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_spoken_mask_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for kind in spoken_mask_kinds() {
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: kind
                    .configuration
                    .iter()
                    .map(|field| conduit_plot::StartupParameterSignature {
                        name: field.key.clone(),
                        value_type: "Count".into(),
                        default: match field.default_value {
                            conduit_core::ConfigurationValue::U64(v) => Some(alloc::format!("{v}")),
                            _ => None,
                        },
                    })
                    .collect(),
            })
            .map_err(|error| alloc::format!("install spoken Mask signature: {error:?}"))?;
        profiles
            .insert_kind(kind)
            .map_err(|error| alloc::format!("install spoken Mask profile: {error:?}"))?;
    }
    Ok(())
}

pub fn spoken_mask_kinds() -> alloc::vec::Vec<conduit_core::Kind> {
    use conduit_core::{
        kind_id, port_id, CapabilityLimits, Kind, KindIdentity, PortDescriptor, PortDirection,
        PortTemporal,
    };
    let port = |name: &str, value_kind: &str, direction: PortDirection, temporal: PortTemporal| {
        PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(value_kind),
            direction,
            temporal,
            abnormal_kind: None,
        }
    };
    let kind = |identity: &str, inputs: alloc::vec::Vec<_>, outputs: alloc::vec::Vec<_>| Kind {
        startup_parameters: alloc::vec::Vec::new(),
        shorthand: None,
        kind_id: kind_id(identity),
        kind_contract_revision: KindIdentity::from(
            if identity == crate::GENERATED_VALIDATOR_KIND {
                crate::GENERATED_VALIDATOR_CONTRACT_REVISION
            } else {
                SPOKEN_MASK_CONTRACT_REVISION
            },
        ),
        inputs,
        outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 2,
            max_queue_bytes: (crate::MAX_GENERATIVE_PRESENTER_INPUT_BYTES * 2) as u32,
        },
    };
    let mut kinds = alloc::vec![
        kind(
            GENERATED_VALIDATION_ENVELOPE_KIND,
            alloc::vec![
                port(
                    "request",
                    crate::GENERATIVE_PRESENTER_INPUT_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
                port(
                    "candidate",
                    crate::GENERATED_MANIFESTATION_CANDIDATE_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
            ],
            alloc::vec![port(
                "envelope",
                crate::GENERATED_VALIDATION_ENVELOPE_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            crate::GENERATED_VALIDATOR_KIND,
            alloc::vec![port(
                "envelope",
                crate::GENERATED_VALIDATION_ENVELOPE_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            alloc::vec![port(
                "assessment",
                crate::GENERATED_VALIDATOR_ASSESSMENT_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            RETAIN_GENERATED_VALIDATION_KIND,
            alloc::vec![
                port(
                    "candidate",
                    crate::GENERATED_MANIFESTATION_CANDIDATE_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
                port(
                    "assessment",
                    crate::GENERATED_VALIDATOR_ASSESSMENT_VALUE_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
            ],
            alloc::vec![port(
                "manifestation",
                crate::GENERATED_MANIFESTATION_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            PRESENTATION_TO_GENERATIVE_REQUEST_KIND,
            alloc::vec![port(
                "presentation",
                crate::PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            alloc::vec![port(
                "request",
                crate::GENERATIVE_PRESENTER_INPUT_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            GENERATED_MANIFESTATION_TO_SPEECH_KIND,
            alloc::vec![port(
                "manifestation",
                crate::GENERATED_MANIFESTATION_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            alloc::vec![port(
                "speech",
                "value/text",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            SPOKEN_ARTIFACT_KIND,
            alloc::vec![port(
                "audio",
                "audio/pcm-frames@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            alloc::vec![port(
                "receipt",
                SPOKEN_MASK_ARTIFACT_RECEIPT_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            ARTIFACT_ACKNOWLEDGED_SHOW_KIND,
            alloc::vec![
                port(
                    "manifestation",
                    crate::GENERATED_MANIFESTATION_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
                port(
                    "artifact",
                    SPOKEN_MASK_ARTIFACT_RECEIPT_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
            ],
            alloc::vec![port(
                "show",
                crate::SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            CLOSING_NO_INTERACTION_KIND,
            alloc::vec![],
            alloc::vec![port(
                "interaction",
                crate::FACE_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
        kind(
            DIRECT_FACE_WORDING_KIND,
            alloc::vec![port(
                "presentation",
                crate::PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            alloc::vec![port(
                "speech",
                "value/text",
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
        kind(
            DIRECT_ARTIFACT_SHOW_KIND,
            alloc::vec![
                port(
                    "presentation",
                    crate::PRESENTATION_VALUE_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
                port(
                    "artifact",
                    SPOKEN_MASK_ARTIFACT_RECEIPT_KIND,
                    PortDirection::Input,
                    PortTemporal::Value,
                ),
            ],
            alloc::vec![port(
                "show",
                crate::SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
    ];
    // A closing Flow is explicit semantic meaning, not an implicit Value lift.
    // Keep the existing single-shot Value projection and its 256-byte contract.
    let mut stream = kind(
        GENERATED_MANIFESTATION_TO_SPEECH_STREAM_KIND,
        alloc::vec![port(
            "manifestation",
            crate::GENERATED_MANIFESTATION_KIND,
            PortDirection::Input,
            PortTemporal::Value
        )],
        alloc::vec![port(
            "speech",
            "value/text",
            PortDirection::Output,
            PortTemporal::Flow { closes: true }
        )],
    );
    stream.kind_contract_revision =
        KindIdentity::from("conduit.presentation/generated-manifestation-speech-stream@1");
    stream
        .semantic_laws
        .push(conduit_core::KindSemanticLaw::ValueContracts(alloc::vec![
            conduit_core::FrontValueContract {
                location: conduit_core::FrontValueLocation::Output(port_id("speech")),
                contract: conduit_core::CheckedValueContract::new(
                    kind_id("value/text"),
                    1024,
                    alloc::vec::Vec::new()
                )
                .expect("finite speech stream item"),
            },
        ]));
    kinds.push(stream);
    let direct = kinds
        .iter_mut()
        .find(|kind| kind.kind_id.as_str() == DIRECT_FACE_WORDING_KIND)
        .expect("direct Face wording Kind");
    direct.kind_contract_revision =
        KindIdentity::from("conduit.presentation/direct-face-wording@1");
    direct
        .semantic_laws
        .push(conduit_core::KindSemanticLaw::ValueContracts(alloc::vec![
            conduit_core::FrontValueContract {
                location: conduit_core::FrontValueLocation::Output(port_id("speech")),
                contract: conduit_core::CheckedValueContract::new(
                    kind_id("value/text"),
                    1024,
                    alloc::vec::Vec::new(),
                )
                .expect("finite direct Face wording item"),
            },
        ]));
    let artifact = kinds
        .iter_mut()
        .find(|kind| kind.kind_id.as_str() == SPOKEN_ARTIFACT_KIND)
        .expect("artifact Kind");
    artifact.kind_contract_revision = KindIdentity::from("conduit.presentation/spoken-artifact@2");
    for (key, default, maximum) in [
        ("maximum-blocks", 3_072, 32_768),
        ("maximum-audio-millis", 16_384, 30_000),
    ] {
        artifact
            .startup_parameters
            .push(conduit_core::FrontStartupParameter {
                name: key.into(),
                value_type: kind_id("value/count"),
                has_default: true,
            });
        artifact
            .configuration
            .push(conduit_core::KindConfigurationField {
                key: key.into(),
                default_value: conduit_core::ConfigurationValue::U64(default),
                rule: conduit_core::KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum,
                },
            });
    }
    kinds
}
