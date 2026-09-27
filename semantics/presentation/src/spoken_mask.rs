//! Exact evidence at the terminal boundary of an artifact-output spoken Mask.
//!
//! Writing an audio artifact is a truthful output effect. It is neither a
//! claim that a speaker played it nor evidence that a human heard it.

use alloc::string::String;
use conduit_core::{ActivePlayId, PlanId, SignId};
use serde::{Deserialize, Serialize};

use crate::{GeneratedManifestation, MaskShow, MaskShowError, Presentation};

pub const SPOKEN_MASK_ARTIFACT_RECEIPT_KIND: &str =
    "conduit.presentation/spoken-mask-artifact-receipt@1";
pub const MAX_SPOKEN_MASK_ARTIFACT_IDENTITY_BYTES: usize = 256;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpokenMaskArtifactReceipt {
    pub artifact_identity: String,
    pub content_sha256: String,
    pub pcm_bytes: u32,
    pub frames: u32,
    pub blocks: u16,
    pub plan_id: PlanId,
    pub active_play_id: ActivePlayId,
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub generated_manifestation_identity: String,
    pub completion_sign_id: SignId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactAcknowledgedSpokenShow {
    pub show: MaskShow,
    pub generated_manifestation_identity: String,
    pub artifact: SpokenMaskArtifactReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpokenMaskShowError {
    InvalidArtifactIdentity,
    EmptyArtifact,
    StalePresentation,
    StaleGeneration,
    StalePlan,
    InvalidShow(MaskShowError),
}

impl ArtifactAcknowledgedSpokenShow {
    pub fn validate(
        &self,
        presentation: &Presentation,
        generated: &GeneratedManifestation,
    ) -> Result<(), SpokenMaskShowError> {
        if self.artifact.artifact_identity.is_empty()
            || self.artifact.artifact_identity.len() > MAX_SPOKEN_MASK_ARTIFACT_IDENTITY_BYTES
            || self.artifact.content_sha256.len() != 64
            || !self
                .artifact
                .content_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(SpokenMaskShowError::InvalidArtifactIdentity);
        }
        if self.artifact.pcm_bytes == 0 || self.artifact.frames == 0 || self.artifact.blocks == 0 {
            return Err(SpokenMaskShowError::EmptyArtifact);
        }
        if self.artifact.source_presentation_identity != presentation.identity.as_str()
            || self.artifact.source_presentation_revision != presentation.revision
        {
            return Err(SpokenMaskShowError::StalePresentation);
        }
        if self.generated_manifestation_identity != generated.manifestation_identity
            || self.artifact.generated_manifestation_identity != generated.manifestation_identity
            || generated.source_presentation_identity != presentation.identity.as_str()
            || generated.source_presentation_revision != presentation.revision
        {
            return Err(SpokenMaskShowError::StaleGeneration);
        }
        if self.artifact.plan_id != self.show.planned_mask.plan.plan_id
            || self.artifact.active_play_id != self.show.show.active_play_id
        {
            return Err(SpokenMaskShowError::StalePlan);
        }
        self.show
            .validate(presentation)
            .map_err(SpokenMaskShowError::InvalidShow)
    }
}
