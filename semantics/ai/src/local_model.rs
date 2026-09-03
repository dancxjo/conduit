//! LLM-oriented hosted model profile.
//!
//! This remains a specialization of the generalized model mechanism: its
//! model content corresponds to a `ModelArtifact`, its finite LLM operations
//! correspond to a subset of `ModelSignature`, and its runtime fields belong
//! to `ModelRuntimeRealization`. Existing LLM Kind identities remain intact;
//! this module is not the provider-neutral model vocabulary.

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    compute_resource_requirement, resource_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ComputeServiceGuarantee, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer,
};
use serde::{Deserialize, Serialize};

use crate::{
    llm_contract, LlmDeterminismProfile, LlmWorkBounds, LLM_CLASSIFY_KIND, LLM_EMBED_KIND,
    LLM_EXTRACT_KIND, LLM_GENERATE_KIND, LLM_INTERPRET_KIND,
};

pub const LOCAL_MODEL_OPERATION: &str = "conduit.host/local-model-inference@1";
pub const LOCAL_MODEL_MEMORY_RESOURCE: &str = "conduit.resource/local-model-memory-mib@1";
pub const LOCAL_MODEL_COMPUTE_RESOURCE: &str = "conduit.resource/compute/shared-lane@1";
pub const LOCAL_MODEL_INFERENCE_SLOT_RESOURCE: &str =
    "conduit.resource/local-model-inference-slot@1";
pub const LOCAL_MODEL_QUEUE_ITEM_RESOURCE: &str = "conduit.resource/local-model-queue-item@1";
pub const LOCAL_MODEL_QUEUE_KIB_RESOURCE: &str = "conduit.resource/local-model-queue-kib@1";
pub const LOCAL_MODEL_IMPLEMENTATION: &str = "std/local-open-weight-model@1";
pub const LOCAL_MODEL_EXECUTION_PROFILE: &str = "conduit.llm/local-model-hosted@1";
pub const LOCAL_MODEL_ARTIFACT: &str = "conduit-std-host/local-model-adapter@1";
pub const LOCAL_MODEL_CAPABILITY_PREFIX: &str = "local-model";
pub const MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES: usize = 256;
pub const MAXIMUM_LOCAL_MODEL_KINDS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelKindProfile {
    Generate,
    ClassifyFiniteLabels,
    ExtractValidatedInfo,
    EmbedFiniteVector,
    InterpretSignEvidence,
}

impl LocalModelKindProfile {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Generate => LLM_GENERATE_KIND,
            Self::ClassifyFiniteLabels => LLM_CLASSIFY_KIND,
            Self::ExtractValidatedInfo => LLM_EXTRACT_KIND,
            Self::EmbedFiniteVector => LLM_EMBED_KIND,
            Self::InterpretSignEvidence => LLM_INTERPRET_KIND,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelCachePolicy {
    OneLoadedModelUntilShutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelLifecycleState {
    Discovered,
    Loading,
    Warming,
    Ready,
    Inference,
    Unloading,
    Shutdown,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelTerminal {
    Produced,
    Truncated,
    Refused(LocalModelRefusal),
    Failed(LocalModelFailure),
    Cancelled,
    ProviderLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelRefusal {
    NotInitialized,
    UnsupportedKind,
    UnsupportedProfile,
    InputOverflow,
    ContextOverflow,
    OutputOverflow,
    QueueFull,
    MemoryCeiling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelFailure {
    Load,
    Warmup,
    MalformedRequest,
    MalformedResult,
    Inference,
    ResourceExhausted,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelIdentity {
    pub runtime_name: String,
    pub runtime_version: String,
    pub runtime_build_identity: String,
    pub model_name: String,
    pub model_content_identity: String,
    pub architecture: String,
    pub parameter_profile: String,
    pub quantization: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelLimits {
    pub work: LlmWorkBounds,
    pub model_bytes: u64,
    pub admitted_memory_mib: u32,
    pub compute: LocalModelComputeNeed,
    pub maximum_in_flight: u16,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
    pub cancellation_supported: bool,
    pub cache_policy: LocalModelCachePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelComputeNeed {
    pub minimum_lanes: u32,
    pub preferred_lanes: u32,
    pub maximum_lanes: u32,
    pub minimum_service_guarantee: ComputeServiceGuarantee,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelOffer {
    pub identity: LocalModelIdentity,
    pub limits: LocalModelLimits,
    pub supported_profiles: Vec<LocalModelKindProfile>,
    pub initialized: bool,
    pub lifecycle: LocalModelLifecycleState,
    pub determinism: LlmDeterminismProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocalModelOfferInvalidity {
    MissingIdentity,
    IdentityOverflow,
    InvalidLimits,
    ModelExceedsMemoryCeiling,
    InvalidConcurrency,
    InvalidQueue,
    MissingProfile,
    DuplicateProfile,
    NotReady,
    DeterministicClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelRequestAdmission {
    pub input_bytes: u64,
    pub context_items: u64,
    pub output_bytes: u64,
    pub work_units: u64,
    pub history_items: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalModelSession {
    offer: LocalModelOffer,
    state: LocalModelLifecycleState,
    active_kind: Option<LocalModelKindProfile>,
}

impl LocalModelOffer {
    pub fn validate(&self) -> Result<(), LocalModelOfferInvalidity> {
        let identities = [
            &self.identity.runtime_name,
            &self.identity.runtime_version,
            &self.identity.runtime_build_identity,
            &self.identity.model_name,
            &self.identity.model_content_identity,
            &self.identity.architecture,
            &self.identity.parameter_profile,
            &self.identity.quantization,
        ];
        if identities.iter().any(|value| value.is_empty()) {
            return Err(LocalModelOfferInvalidity::MissingIdentity);
        }
        if identities
            .iter()
            .any(|value| value.len() > MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES)
        {
            return Err(LocalModelOfferInvalidity::IdentityOverflow);
        }
        if !self.limits.work.valid()
            || self.limits.model_bytes == 0
            || self.limits.admitted_memory_mib == 0
            || self.limits.compute.minimum_lanes == 0
            || self.limits.compute.minimum_lanes > self.limits.compute.preferred_lanes
            || self.limits.compute.preferred_lanes > self.limits.compute.maximum_lanes
        {
            return Err(LocalModelOfferInvalidity::InvalidLimits);
        }
        if self.limits.model_bytes
            > u64::from(self.limits.admitted_memory_mib).saturating_mul(1024 * 1024)
        {
            return Err(LocalModelOfferInvalidity::ModelExceedsMemoryCeiling);
        }
        if self.limits.maximum_in_flight != 1 {
            return Err(LocalModelOfferInvalidity::InvalidConcurrency);
        }
        if self.limits.maximum_queue_items == 0
            || u64::from(self.limits.maximum_queue_bytes)
                < self
                    .limits
                    .work
                    .maximum_input_bytes
                    .saturating_mul(u64::from(self.limits.maximum_queue_items))
        {
            return Err(LocalModelOfferInvalidity::InvalidQueue);
        }
        if self.supported_profiles.is_empty()
            || self.supported_profiles.len() > MAXIMUM_LOCAL_MODEL_KINDS
        {
            return Err(LocalModelOfferInvalidity::MissingProfile);
        }
        if self
            .supported_profiles
            .iter()
            .enumerate()
            .any(|(index, profile)| self.supported_profiles[index + 1..].contains(profile))
        {
            return Err(LocalModelOfferInvalidity::DuplicateProfile);
        }
        if !self.initialized || self.lifecycle != LocalModelLifecycleState::Ready {
            return Err(LocalModelOfferInvalidity::NotReady);
        }
        if self.determinism.permits_semantic_output_equality_claim() {
            return Err(LocalModelOfferInvalidity::DeterministicClaim);
        }
        Ok(())
    }

    pub fn capability_offers(&self) -> Result<Vec<CapabilityOffer>, LocalModelOfferInvalidity> {
        self.validate()?;
        Ok(self
            .supported_profiles
            .iter()
            .copied()
            .map(|profile| self.capability_offer(profile))
            .collect())
    }

    fn capability_offer(&self, profile: LocalModelKindProfile) -> CapabilityOffer {
        let contract = llm_contract(profile.kind()).expect("local profiles name catalogued kinds");
        let operation = HostOperationRequirement {
            contract_id: HostOperationContractId::from(LOCAL_MODEL_OPERATION),
            target_kind: Some(contract.kind_id.clone()),
            maximum_in_flight: self.limits.maximum_in_flight,
            maximum_input_bytes: self.limits.work.maximum_input_bytes as u32,
            maximum_output_bytes: self.limits.work.maximum_output_bytes as u32,
        };
        let mut resource_requirements = vec![
            resource_requirement(LOCAL_MODEL_MEMORY_RESOURCE, self.limits.admitted_memory_mib),
            compute_resource_requirement(
                LOCAL_MODEL_COMPUTE_RESOURCE,
                self.limits.compute.minimum_lanes,
                self.limits.compute.preferred_lanes,
                self.limits.compute.maximum_lanes,
                self.limits.compute.minimum_service_guarantee,
                None,
            ),
            resource_requirement(
                LOCAL_MODEL_INFERENCE_SLOT_RESOURCE,
                u32::from(self.limits.maximum_in_flight),
            ),
            resource_requirement(
                LOCAL_MODEL_QUEUE_ITEM_RESOURCE,
                u32::from(self.limits.maximum_queue_items),
            ),
            resource_requirement(
                LOCAL_MODEL_QUEUE_KIB_RESOURCE,
                self.limits.maximum_queue_bytes.div_ceil(1024),
            ),
        ];
        resource_requirements.sort();
        CapabilityOffer {
            startup_parameters: [
                "maximum-input-bytes",
                "maximum-context-items",
                "maximum-output-bytes",
                "maximum-work-units",
                "maximum-history-items",
            ]
            .into_iter()
            .map(|name| conduit_core::FaceStartupParameter {
                name: name.into(),
                value_type: "Count".into(),
                has_default: true,
            })
            .collect(),
            shorthand: None,
            capability_id: CapabilityId::from(alloc::format!(
                "{LOCAL_MODEL_CAPABILITY_PREFIX}/{}",
                profile.kind()
            )),
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(LOCAL_MODEL_EXECUTION_PROFILE),
                implementation_id: ImplementationId::from(LOCAL_MODEL_IMPLEMENTATION),
                artifact_id: ArtifactId::from(alloc::format!(
                    "{LOCAL_MODEL_ARTIFACT}/{}",
                    self.identity.model_content_identity
                )),
            },
            host_operations: vec![operation],
            resource_requirements,
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: self.limits.maximum_in_flight,
                max_queue_items: self.limits.maximum_queue_items,
                max_queue_bytes: self.limits.maximum_queue_bytes,
            },
        }
    }
}

impl LocalModelSession {
    pub fn new(offer: LocalModelOffer) -> Result<Self, LocalModelOfferInvalidity> {
        offer.validate()?;
        Ok(Self {
            offer,
            state: LocalModelLifecycleState::Ready,
            active_kind: None,
        })
    }

    pub const fn state(&self) -> LocalModelLifecycleState {
        self.state
    }

    pub fn admit(
        &mut self,
        profile: LocalModelKindProfile,
        request: LocalModelRequestAdmission,
    ) -> Result<(), LocalModelRefusal> {
        if self.state == LocalModelLifecycleState::Lost {
            return Err(LocalModelRefusal::NotInitialized);
        }
        if self.state != LocalModelLifecycleState::Ready || self.active_kind.is_some() {
            return Err(LocalModelRefusal::QueueFull);
        }
        if !self.offer.supported_profiles.contains(&profile) {
            return Err(LocalModelRefusal::UnsupportedKind);
        }
        let bounds = self.offer.limits.work;
        if request.input_bytes > bounds.maximum_input_bytes {
            return Err(LocalModelRefusal::InputOverflow);
        }
        if request.context_items > bounds.maximum_context_items
            || request.history_items > bounds.maximum_history_items
        {
            return Err(LocalModelRefusal::ContextOverflow);
        }
        if request.output_bytes > bounds.maximum_output_bytes {
            return Err(LocalModelRefusal::OutputOverflow);
        }
        if request.work_units > bounds.maximum_work_units {
            return Err(LocalModelRefusal::UnsupportedProfile);
        }
        self.active_kind = Some(profile);
        self.state = LocalModelLifecycleState::Inference;
        Ok(())
    }

    pub fn finish(&mut self, terminal: LocalModelTerminal) -> Result<(), LocalModelFailure> {
        if self.state != LocalModelLifecycleState::Inference || self.active_kind.is_none() {
            return Err(LocalModelFailure::Inference);
        }
        self.active_kind = None;
        self.state = match terminal {
            LocalModelTerminal::ProviderLost => LocalModelLifecycleState::Lost,
            _ => LocalModelLifecycleState::Ready,
        };
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<LocalModelTerminal, LocalModelFailure> {
        if self.state != LocalModelLifecycleState::Inference || self.active_kind.is_none() {
            return Err(LocalModelFailure::Inference);
        }
        self.active_kind = None;
        self.state = LocalModelLifecycleState::Ready;
        Ok(LocalModelTerminal::Cancelled)
    }

    pub fn provider_lost(&mut self) -> LocalModelTerminal {
        self.active_kind = None;
        self.state = LocalModelLifecycleState::Lost;
        LocalModelTerminal::ProviderLost
    }

    pub fn shutdown(&mut self) -> Result<(), LocalModelFailure> {
        if self.state == LocalModelLifecycleState::Inference {
            return Err(LocalModelFailure::Shutdown);
        }
        self.active_kind = None;
        self.state = LocalModelLifecycleState::Shutdown;
        Ok(())
    }
}
