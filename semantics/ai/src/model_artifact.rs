//! Exact model content, mutable state, checkpoints, and Host realizations.

use alloc::{string::String, vec::Vec};
use conduit_core::{semantic_digest, BoundedResourceRef};

use crate::{ModelOperation, ModelSignature, ModelSignatureRefusal, MAXIMUM_MODEL_IDENTITY_BYTES};

pub const MODEL_ARTIFACT_INFO_ID: &str = "model/artifact@1";
pub const MODEL_CHECKPOINT_INFO_ID: &str = "model/checkpoint@1";
pub const MODEL_CONTENT_INFO_ID: &str = "model/content@1";
pub const MAXIMUM_RUNTIME_PROFILES: usize = 16;
pub const MAXIMUM_MODEL_INPUT_IDENTITIES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelArtifact {
    pub architecture_profile: String,
    pub format_profile: String,
    pub precision_profile: String,
    pub state_schema_version: u32,
    pub signature_identity: [u8; 32],
    pub content: BoundedResourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutableModelState {
    pub base_artifact_identity: [u8; 32],
    pub state_identity: String,
    pub state_schema_version: u32,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCheckpoint {
    pub base_artifact_identity: [u8; 32],
    pub architecture_profile: String,
    pub state_schema_version: u32,
    pub generation: u64,
    pub content: BoundedResourceRef,
}

/// Host-local execution evidence. Framework and device facts live here, not
/// in the portable model artifact or signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRuntimeRealization {
    pub implementation_identity: String,
    pub runtime_name: String,
    pub runtime_version: String,
    pub runtime_build_identity: String,
    pub device_profile: String,
    pub supported_formats: Vec<String>,
    pub supported_precisions: Vec<String>,
    pub loaded_artifact_identity: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInvocationEvidence {
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub signature_identity: [u8; 32],
    pub runtime_implementation_identity: String,
    pub runtime_build_identity: String,
    pub precision_profile: String,
    pub operation: ModelOperation,
    pub input_identities: Vec<[u8; 32]>,
    pub stochastic_seed: Option<u64>,
    pub admitted_work_units: u64,
    pub terminal: ModelInvocationTerminal,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelInvocationTerminal {
    Produced,
    Refused(ModelCompatibilityRefusal),
    Failed,
    Cancelled,
    RuntimeLost,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelEvidenceRefusal {
    MissingArtifactIdentity,
    MissingCheckpointIdentity,
    MissingSignatureIdentity,
    InvalidRuntimeIdentity,
    InvalidPrecisionProfile,
    MissingInputIdentity,
    TooManyInputIdentities,
    WorkNotAdmitted,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelCompatibilityRefusal {
    InvalidArtifact,
    InvalidSignature,
    SignatureMismatch,
    InvalidState,
    InvalidCheckpoint,
    ArtifactMismatch,
    ArchitectureMismatch,
    StateSchemaMismatch,
    CorruptContentIdentity,
    InvalidRuntime,
    UnsupportedFormat,
    UnsupportedPrecision,
    RuntimeLoadedDifferentArtifact,
}

impl ModelArtifact {
    pub fn validate(&self, signature: &ModelSignature) -> Result<(), ModelCompatibilityRefusal> {
        validate_identity(&self.architecture_profile)
            .and_then(|()| validate_identity(&self.format_profile))
            .and_then(|()| validate_identity(&self.precision_profile))
            .map_err(|_| ModelCompatibilityRefusal::InvalidArtifact)?;
        if self.state_schema_version == 0 {
            return Err(ModelCompatibilityRefusal::InvalidArtifact);
        }
        self.content
            .validate()
            .map_err(|_| ModelCompatibilityRefusal::InvalidArtifact)?;
        if self.content.extent.bytes == 0
            || self.content.content_profile.as_str() != self.format_profile
        {
            return Err(ModelCompatibilityRefusal::InvalidArtifact);
        }
        let actual = signature
            .semantic_digest()
            .map_err(|_| ModelCompatibilityRefusal::InvalidSignature)?;
        if actual != self.signature_identity {
            return Err(ModelCompatibilityRefusal::SignatureMismatch);
        }
        Ok(())
    }

    pub fn content_identity(&self) -> [u8; 32] {
        self.content.identity.digest()
    }

    pub fn descriptor_digest(
        &self,
        signature: &ModelSignature,
    ) -> Result<[u8; 32], ModelCompatibilityRefusal> {
        self.validate(signature)?;
        let mut bytes = Vec::new();
        push_text(&mut bytes, &self.architecture_profile);
        push_text(&mut bytes, &self.format_profile);
        push_text(&mut bytes, &self.precision_profile);
        bytes.extend_from_slice(&self.state_schema_version.to_le_bytes());
        bytes.extend_from_slice(&self.signature_identity);
        bytes.extend_from_slice(&self.content_identity());
        Ok(semantic_digest(MODEL_ARTIFACT_INFO_ID, &bytes))
    }
}

pub fn model_content_digest(bytes: &[u8]) -> [u8; 32] {
    semantic_digest(MODEL_CONTENT_INFO_ID, bytes)
}

impl MutableModelState {
    pub fn validate(&self, artifact: &ModelArtifact) -> Result<(), ModelCompatibilityRefusal> {
        validate_nonzero(self.base_artifact_identity)
            .map_err(|_| ModelCompatibilityRefusal::InvalidState)?;
        validate_identity(&self.state_identity)
            .map_err(|_| ModelCompatibilityRefusal::InvalidState)?;
        if self.base_artifact_identity != artifact.content_identity() {
            return Err(ModelCompatibilityRefusal::ArtifactMismatch);
        }
        if self.state_schema_version != artifact.state_schema_version {
            return Err(ModelCompatibilityRefusal::StateSchemaMismatch);
        }
        Ok(())
    }
}

impl ModelCheckpoint {
    pub fn validate(&self, artifact: &ModelArtifact) -> Result<(), ModelCompatibilityRefusal> {
        self.content
            .validate()
            .map_err(|_| ModelCompatibilityRefusal::InvalidCheckpoint)?;
        validate_identity(&self.architecture_profile)
            .map_err(|_| ModelCompatibilityRefusal::InvalidCheckpoint)?;
        if self.content.extent.bytes == 0
            || self.content.content_profile.as_str() != MODEL_CHECKPOINT_INFO_ID
        {
            return Err(ModelCompatibilityRefusal::InvalidCheckpoint);
        }
        if self.content.identity.digest() == artifact.content_identity() {
            return Err(ModelCompatibilityRefusal::CorruptContentIdentity);
        }
        if self.base_artifact_identity != artifact.content_identity() {
            return Err(ModelCompatibilityRefusal::ArtifactMismatch);
        }
        if self.architecture_profile != artifact.architecture_profile {
            return Err(ModelCompatibilityRefusal::ArchitectureMismatch);
        }
        if self.state_schema_version != artifact.state_schema_version {
            return Err(ModelCompatibilityRefusal::StateSchemaMismatch);
        }
        Ok(())
    }
}

impl ModelRuntimeRealization {
    pub fn admit(&self, artifact: &ModelArtifact) -> Result<(), ModelCompatibilityRefusal> {
        for identity in [
            &self.implementation_identity,
            &self.runtime_name,
            &self.runtime_version,
            &self.runtime_build_identity,
            &self.device_profile,
        ] {
            validate_identity(identity).map_err(|_| ModelCompatibilityRefusal::InvalidRuntime)?;
        }
        if self.supported_formats.is_empty()
            || self.supported_formats.len() > MAXIMUM_RUNTIME_PROFILES
            || self.supported_precisions.is_empty()
            || self.supported_precisions.len() > MAXIMUM_RUNTIME_PROFILES
        {
            return Err(ModelCompatibilityRefusal::InvalidRuntime);
        }
        if !self.supported_formats.contains(&artifact.format_profile) {
            return Err(ModelCompatibilityRefusal::UnsupportedFormat);
        }
        if !self
            .supported_precisions
            .contains(&artifact.precision_profile)
        {
            return Err(ModelCompatibilityRefusal::UnsupportedPrecision);
        }
        if self.loaded_artifact_identity != artifact.content_identity() {
            return Err(ModelCompatibilityRefusal::RuntimeLoadedDifferentArtifact);
        }
        Ok(())
    }
}

impl ModelInvocationEvidence {
    pub fn validate(&self) -> Result<(), ModelEvidenceRefusal> {
        validate_nonzero(self.artifact_identity)
            .map_err(|_| ModelEvidenceRefusal::MissingArtifactIdentity)?;
        if self
            .checkpoint_identity
            .is_some_and(|identity| identity == [0; 32])
        {
            return Err(ModelEvidenceRefusal::MissingCheckpointIdentity);
        }
        validate_nonzero(self.signature_identity)
            .map_err(|_| ModelEvidenceRefusal::MissingSignatureIdentity)?;
        validate_identity(&self.runtime_implementation_identity)
            .and_then(|()| validate_identity(&self.runtime_build_identity))
            .map_err(|_| ModelEvidenceRefusal::InvalidRuntimeIdentity)?;
        validate_identity(&self.precision_profile)
            .map_err(|_| ModelEvidenceRefusal::InvalidPrecisionProfile)?;
        if self.input_identities.is_empty() || self.input_identities.contains(&[0; 32]) {
            return Err(ModelEvidenceRefusal::MissingInputIdentity);
        }
        if self.input_identities.len() > MAXIMUM_MODEL_INPUT_IDENTITIES {
            return Err(ModelEvidenceRefusal::TooManyInputIdentities);
        }
        if self.admitted_work_units == 0 {
            return Err(ModelEvidenceRefusal::WorkNotAdmitted);
        }
        Ok(())
    }
}

fn validate_identity(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.len() > MAXIMUM_MODEL_IDENTITY_BYTES {
        Err(())
    } else {
        Ok(())
    }
}

fn validate_nonzero(value: [u8; 32]) -> Result<(), ()> {
    if value == [0; 32] {
        Err(())
    } else {
        Ok(())
    }
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u16).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

impl From<ModelSignatureRefusal> for ModelCompatibilityRefusal {
    fn from(_: ModelSignatureRefusal) -> Self {
        Self::InvalidSignature
    }
}
