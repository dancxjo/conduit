//! Candidate output and accountable semantic validation for generative Masks.

use crate::GenerativePresenterRefusal;
use alloc::{string::String, vec::Vec};
use conduit_core::{BootId, CapabilityId, HostId, ImplementationId, PlacementId, PlanId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_GENERATED_CONTENT_SEGMENTS: usize = 8;
pub const MAX_GENERATED_AFFORDANCES: usize = 32;
pub const MAX_GENERATED_CORRELATIONS: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GeneratedManifestationDisposition {
    Produced,
    Truncated,
    Refused,
    Failed,
    Cancelled,
    ProviderLost,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedActionAffordance {
    pub action_identity: String,
    pub source_presentation_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GeneratedContentRole {
    Speech,
    PresentedThought,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedContentSegment {
    pub role: GeneratedContentRole,
    /// Exact text statement in the correlated source Face. This is selection,
    /// not a claim that a validator can prove arbitrary prose entailment.
    pub source_text_index: u32,
    pub bytes: Vec<u8>,
}

/// An exact claim that prose is grounded in one element of the immutable source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum GeneratedSemanticCorrelation {
    Subject {
        index: u32,
        identity: String,
    },
    Relationship {
        index: u32,
        source: String,
        target: String,
        kind: crate::PresentationRelationshipKind,
    },
    Text {
        index: u32,
        subject: String,
    },
    Property {
        index: u32,
        subject: String,
        name: String,
    },
    TypedContent {
        index: u32,
        subject: String,
        name: String,
        content_profile: String,
    },
    Composition {
        index: u32,
        identity: String,
    },
    Action {
        index: u32,
        identity: String,
        intent: String,
        target: String,
    },
    ActionArgument {
        action_index: u32,
        argument_index: u32,
        name: String,
        value_kind: String,
    },
    Disclosure {
        index: u32,
        subject: String,
        level: crate::PresentationDisclosureLevel,
    },
    TemporalReference {
        index: u32,
        identity: String,
    },
    TemporalFact {
        index: u32,
        subject: String,
        reference: String,
        role: crate::PresentationTemporalRole,
    },
    Context {
        index: u32,
        source: String,
        target: String,
        relationship: crate::PresentationRelationshipKind,
    },
}

/// Untrusted provider output. This is deliberately not a Manifestation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedManifestationCandidate {
    pub candidate_identity: String,
    pub request_identity: String,
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub presenter_implementation_identity: String,
    pub provider_identity: String,
    pub model_identity: String,
    pub template_contract_revision: String,
    pub mask_contract_revision: String,
    pub generation_run_identity: String,
    pub disposition: GeneratedManifestationDisposition,
    pub content: Vec<GeneratedContentSegment>,
    pub affordances: Vec<GeneratedActionAffordance>,
    pub correlations: Vec<GeneratedSemanticCorrelation>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GeneratedValidationDisposition {
    Accepted,
    Refused,
    Failed,
    Cancelled,
    ValidatorLost,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedValidationReceipt {
    pub(crate) receipt_identity: String,
    pub(crate) plan_id: PlanId,
    pub(crate) placement_id: PlacementId,
    pub(crate) validator_implementation_identity: ImplementationId,
    pub(crate) validator_back_identity: CapabilityId,
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) assessment_identity: String,
    pub(crate) candidate_digest: String,
    pub(crate) source_presentation_identity: String,
    pub(crate) source_presentation_revision: u64,
    pub(crate) mask_identity: String,
    pub(crate) mask_contract_revision: String,
    pub(crate) accepted_correlations: Vec<GeneratedSemanticCorrelation>,
    pub(crate) disposition: GeneratedValidationDisposition,
    pub(crate) terminal_code: Option<String>,
}

/// Output which crossed the validation boundary. Construction is private.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedManifestation {
    candidate: GeneratedManifestationCandidate,
    receipt: GeneratedValidationReceipt,
}

impl GeneratedManifestation {
    #[allow(dead_code)]
    pub(crate) fn validated(
        candidate: GeneratedManifestationCandidate,
        receipt: GeneratedValidationReceipt,
    ) -> Self {
        Self { candidate, receipt }
    }
    pub fn candidate(&self) -> &GeneratedManifestationCandidate {
        &self.candidate
    }
    pub fn validation_receipt(&self) -> &GeneratedValidationReceipt {
        &self.receipt
    }
    pub fn manifestation_identity(&self) -> &str {
        &self.candidate.candidate_identity
    }
    pub fn source_presentation_identity(&self) -> &str {
        &self.candidate.source_presentation_identity
    }
    pub fn source_presentation_revision(&self) -> u64 {
        self.candidate.source_presentation_revision
    }
    pub fn content(&self) -> &[GeneratedContentSegment] {
        &self.candidate.content
    }
    pub fn affordances(&self) -> &[GeneratedActionAffordance] {
        &self.candidate.affordances
    }
}

impl GeneratedValidationReceipt {
    pub fn receipt_identity(&self) -> &str {
        &self.receipt_identity
    }
    pub fn assessment_identity(&self) -> &str {
        &self.assessment_identity
    }
    pub fn accepted_correlations(&self) -> &[GeneratedSemanticCorrelation] {
        &self.accepted_correlations
    }
    pub fn validator_implementation_identity(&self) -> &str {
        self.validator_implementation_identity.as_str()
    }
    pub fn validator_back_identity(&self) -> &str {
        self.validator_back_identity.as_str()
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }
    pub fn placement_id(&self) -> &PlacementId {
        &self.placement_id
    }
    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }
    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }
    pub fn candidate_digest(&self) -> &str {
        &self.candidate_digest
    }
    pub fn source_presentation_identity(&self) -> &str {
        &self.source_presentation_identity
    }
    pub fn source_presentation_revision(&self) -> u64 {
        self.source_presentation_revision
    }
    pub fn mask_identity(&self) -> &str {
        &self.mask_identity
    }
    pub fn mask_contract_revision(&self) -> &str {
        &self.mask_contract_revision
    }
    pub fn disposition(&self) -> GeneratedValidationDisposition {
        self.disposition
    }
    pub fn terminal_code(&self) -> Option<&str> {
        self.terminal_code.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneratedValidationError {
    InvalidCandidate(GenerativePresenterRefusal),
    InvalidValidatorIdentity,
    InvalidAssessmentIdentity,
    InvalidMaskIdentity,
    CandidateDigestMismatch,
    PresentationMismatch,
    MaskMismatch,
    RefusalContract,
    NotAccepted,
    InvalidAcceptedCorrelations,
    InvalidAssessmentContract,
    InvalidPlan,
    MissingValidatorPlacement,
    WrongValidatorKind,
    StaleValidatorBinding,
}

impl GeneratedManifestationCandidate {
    pub fn digest(&self) -> String {
        let mut state = Sha256::new();
        state.update(b"conduit.presentation/generated-manifestation-candidate@2\0");
        for value in [
            &self.request_identity,
            &self.source_presentation_identity,
            &self.presenter_implementation_identity,
            &self.provider_identity,
            &self.model_identity,
            &self.template_contract_revision,
            &self.mask_contract_revision,
            &self.generation_run_identity,
        ] {
            hash_bytes(&mut state, value.as_bytes());
        }
        state.update(self.source_presentation_revision.to_be_bytes());
        state.update([self.disposition as u8]);
        for segment in &self.content {
            state.update([segment.role as u8]);
            state.update(segment.source_text_index.to_be_bytes());
            hash_bytes(&mut state, &segment.bytes);
        }
        for affordance in &self.affordances {
            hash_bytes(&mut state, affordance.action_identity.as_bytes());
            state.update(affordance.source_presentation_revision.to_be_bytes());
        }
        for correlation in &self.correlations {
            crate::generated_correlation::hash_correlation(&mut state, correlation);
        }
        let digest = state.finalize();
        let mut output = String::with_capacity(71);
        output.push_str("sha256:");
        for byte in digest {
            use core::fmt::Write;
            write!(&mut output, "{byte:02x}").expect("string write");
        }
        output
    }
}

fn hash_bytes(state: &mut Sha256, bytes: &[u8]) {
    state.update((bytes.len() as u64).to_be_bytes());
    state.update(bytes);
}
