//! General Host-side model-compute offers, admission, and lifecycle.

use alloc::{string::String, vec::Vec};
use conduit_core::ComputeServiceGuarantee;
use conduit_data::TensorElement;

pub const MAXIMUM_MODEL_COMPUTE_PROFILES: usize = 16;
pub const MAXIMUM_MODEL_COMPUTE_FORMATS: usize = 16;
pub const MAXIMUM_MODEL_COMPUTE_DTYPES: usize = 16;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelComputeOperation {
    Inference,
    Encode,
    Decode,
    Sample,
    Score,
    TrainStep,
    Evaluate,
    Checkpoint,
    IntegrateDynamics,
    RelationQuery,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PortableComputeClass {
    GeneralCpu,
    VectorCompute,
    Accelerator,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelCachePolicy {
    NoCache,
    Bounded {
        maximum_loaded_models: u16,
        maximum_loaded_bytes: u64,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ComputeCapacity {
    pub class: PortableComputeClass,
    pub minimum_lanes: u32,
    pub preferred_lanes: u32,
    pub maximum_lanes: u32,
    pub service: ComputeServiceGuarantee,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ModelComputeLimits {
    pub maximum_model_bytes: u64,
    pub maximum_working_memory_bytes: u64,
    pub maximum_device_memory_bytes: u64,
    pub maximum_input_bytes: u64,
    pub maximum_output_bytes: u64,
    pub maximum_batch_items: u32,
    pub maximum_rank: u8,
    pub maximum_in_flight: u16,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u64,
    pub cancellation_supported: bool,
    pub compute: ComputeCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeOffer {
    pub identity: String,
    pub supported_operations: Vec<ModelComputeOperation>,
    pub accepted_formats: Vec<String>,
    pub supported_elements: Vec<TensorElement>,
    pub solver_profiles: Vec<String>,
    pub determinism_profiles: Vec<String>,
    pub checkpoint_loading: bool,
    pub checkpoint_writing: bool,
    pub limits: ModelComputeLimits,
    pub cache_policy: ModelCachePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeRequirement {
    pub operation: ModelComputeOperation,
    pub model_format: String,
    pub element: TensorElement,
    pub rank: u8,
    pub model_bytes: u64,
    pub working_memory_bytes: u64,
    pub device_memory_bytes: u64,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub batch_items: u32,
    pub compute_class: PortableComputeClass,
    pub minimum_lanes: u32,
    pub preferred_lanes: u32,
    pub maximum_lanes: u32,
    pub minimum_service: ComputeServiceGuarantee,
    pub solver_profile: Option<String>,
    pub determinism_profile: String,
    pub requires_checkpoint_load: bool,
    pub requires_checkpoint_write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeRuntimeIdentity {
    pub provider_name: String,
    pub runtime_name: String,
    pub runtime_version: String,
    pub runtime_build_identity: String,
    pub adapter_artifact_identity: String,
    pub device_evidence: String,
    pub precision_profile: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelComputeLifecycle {
    Discovered,
    Loading,
    Warming,
    Ready,
    Active(ModelComputeOperation),
    Unloading,
    Lost,
    Failed,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelComputeSession {
    offer: ModelComputeOffer,
    runtime: ModelComputeRuntimeIdentity,
    state: ModelComputeLifecycle,
    loaded_model_identity: Option<[u8; 32]>,
    loaded_model_bytes: u64,
    queued_items: u16,
    queued_bytes: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ModelComputeRefusal {
    InvalidOffer,
    MissingIdentity,
    UnsupportedOperation,
    UnsupportedFormat,
    UnsupportedElement,
    UnsupportedShape,
    UnsupportedComputeClass,
    UnsupportedSolver,
    UnsupportedDeterminism,
    UnsupportedCheckpoint,
    ResourceBoundExceeded,
    QueueFull,
    CancellationUnsupported,
    InvalidLifecycleTransition,
    ProviderUnavailable,
}

impl ModelComputeOffer {
    pub fn validate(&self) -> Result<(), ModelComputeRefusal> {
        text(&self.identity)?;
        if self.supported_operations.is_empty()
            || self.supported_operations.len() > MAXIMUM_MODEL_COMPUTE_PROFILES
            || duplicate(&self.supported_operations)
            || self.accepted_formats.is_empty()
            || self.accepted_formats.len() > MAXIMUM_MODEL_COMPUTE_FORMATS
            || duplicate(&self.accepted_formats)
            || self.supported_elements.is_empty()
            || self.supported_elements.len() > MAXIMUM_MODEL_COMPUTE_DTYPES
            || duplicate(&self.supported_elements)
            || duplicate(&self.solver_profiles)
            || self.determinism_profiles.is_empty()
            || duplicate(&self.determinism_profiles)
        {
            return Err(ModelComputeRefusal::InvalidOffer);
        }
        for value in self
            .accepted_formats
            .iter()
            .chain(&self.solver_profiles)
            .chain(&self.determinism_profiles)
        {
            text(value)?;
        }
        self.limits.validate()?;
        match self.cache_policy {
            ModelCachePolicy::NoCache => {}
            ModelCachePolicy::Bounded {
                maximum_loaded_models,
                maximum_loaded_bytes,
            } if maximum_loaded_models > 0
                && maximum_loaded_bytes >= self.limits.maximum_model_bytes => {}
            ModelCachePolicy::Bounded { .. } => return Err(ModelComputeRefusal::InvalidOffer),
        }
        Ok(())
    }

    pub fn admits(&self, requirement: &ModelComputeRequirement) -> Result<(), ModelComputeRefusal> {
        self.validate()?;
        requirement.validate()?;
        if !self.supported_operations.contains(&requirement.operation) {
            return Err(ModelComputeRefusal::UnsupportedOperation);
        }
        if !self.accepted_formats.contains(&requirement.model_format) {
            return Err(ModelComputeRefusal::UnsupportedFormat);
        }
        if !self.supported_elements.contains(&requirement.element) {
            return Err(ModelComputeRefusal::UnsupportedElement);
        }
        if requirement.rank > self.limits.maximum_rank {
            return Err(ModelComputeRefusal::UnsupportedShape);
        }
        if requirement.compute_class != self.limits.compute.class
            || requirement.minimum_lanes < self.limits.compute.minimum_lanes
            || requirement.preferred_lanes > self.limits.compute.preferred_lanes
            || requirement.maximum_lanes > self.limits.compute.maximum_lanes
            || requirement.minimum_service > self.limits.compute.service
        {
            return Err(ModelComputeRefusal::UnsupportedComputeClass);
        }
        if requirement
            .solver_profile
            .as_ref()
            .is_some_and(|value| !self.solver_profiles.contains(value))
        {
            return Err(ModelComputeRefusal::UnsupportedSolver);
        }
        if !self
            .determinism_profiles
            .contains(&requirement.determinism_profile)
        {
            return Err(ModelComputeRefusal::UnsupportedDeterminism);
        }
        if requirement.requires_checkpoint_load && !self.checkpoint_loading
            || requirement.requires_checkpoint_write && !self.checkpoint_writing
        {
            return Err(ModelComputeRefusal::UnsupportedCheckpoint);
        }
        let limits = self.limits;
        if requirement.model_bytes > limits.maximum_model_bytes
            || requirement.working_memory_bytes > limits.maximum_working_memory_bytes
            || requirement.device_memory_bytes > limits.maximum_device_memory_bytes
            || requirement.input_bytes > limits.maximum_input_bytes
            || requirement.output_bytes > limits.maximum_output_bytes
            || requirement.batch_items > limits.maximum_batch_items
        {
            return Err(ModelComputeRefusal::ResourceBoundExceeded);
        }
        Ok(())
    }
}

impl ModelComputeLimits {
    fn validate(self) -> Result<(), ModelComputeRefusal> {
        if self.maximum_model_bytes == 0
            || self.maximum_working_memory_bytes == 0
            || self.maximum_input_bytes == 0
            || self.maximum_output_bytes == 0
            || self.maximum_batch_items == 0
            || self.maximum_rank == 0
            || self.maximum_in_flight != 1
            || self.maximum_queue_items == 0
            || self.maximum_queue_bytes < self.maximum_input_bytes
            || self.compute.minimum_lanes == 0
            || self.compute.minimum_lanes > self.compute.preferred_lanes
            || self.compute.preferred_lanes > self.compute.maximum_lanes
        {
            Err(ModelComputeRefusal::InvalidOffer)
        } else {
            Ok(())
        }
    }
}

impl ModelComputeRequirement {
    fn validate(&self) -> Result<(), ModelComputeRefusal> {
        text(&self.model_format)?;
        text(&self.determinism_profile)?;
        if self.rank == 0
            || self.model_bytes == 0
            || self.working_memory_bytes == 0
            || self.input_bytes == 0
            || self.output_bytes == 0
            || self.batch_items == 0
            || self.minimum_lanes == 0
            || self.minimum_lanes > self.preferred_lanes
            || self.preferred_lanes > self.maximum_lanes
        {
            return Err(ModelComputeRefusal::ResourceBoundExceeded);
        }
        if let Some(profile) = &self.solver_profile {
            text(profile)?;
        }
        Ok(())
    }
}

pub fn select_model_compute_offer<'a>(
    offers: &'a [ModelComputeOffer],
    requirement: &ModelComputeRequirement,
) -> Result<&'a ModelComputeOffer, ModelComputeRefusal> {
    offers
        .iter()
        .find(|offer| offer.admits(requirement).is_ok())
        .ok_or(ModelComputeRefusal::ProviderUnavailable)
}

impl ModelComputeSession {
    pub fn discovered(
        offer: ModelComputeOffer,
        runtime: ModelComputeRuntimeIdentity,
    ) -> Result<Self, ModelComputeRefusal> {
        offer.validate()?;
        runtime.validate()?;
        Ok(Self {
            offer,
            runtime,
            state: ModelComputeLifecycle::Discovered,
            loaded_model_identity: None,
            loaded_model_bytes: 0,
            queued_items: 0,
            queued_bytes: 0,
        })
    }
    pub const fn state(&self) -> ModelComputeLifecycle {
        self.state
    }
    pub fn runtime(&self) -> &ModelComputeRuntimeIdentity {
        &self.runtime
    }
    pub const fn loaded_model_identity(&self) -> Option<[u8; 32]> {
        self.loaded_model_identity
    }
    pub fn begin_load(
        &mut self,
        identity: [u8; 32],
        bytes: u64,
    ) -> Result<(), ModelComputeRefusal> {
        if self.state != ModelComputeLifecycle::Discovered
            || identity == [0; 32]
            || bytes == 0
            || bytes > self.offer.limits.maximum_model_bytes
        {
            return Err(ModelComputeRefusal::InvalidLifecycleTransition);
        }
        self.loaded_model_identity = Some(identity);
        self.loaded_model_bytes = bytes;
        self.state = ModelComputeLifecycle::Loading;
        Ok(())
    }
    pub fn begin_warming(&mut self) -> Result<(), ModelComputeRefusal> {
        self.transition(
            ModelComputeLifecycle::Loading,
            ModelComputeLifecycle::Warming,
        )
    }
    pub fn ready(&mut self) -> Result<(), ModelComputeRefusal> {
        self.transition(ModelComputeLifecycle::Warming, ModelComputeLifecycle::Ready)
    }
    pub fn enqueue(&mut self, bytes: u64) -> Result<(), ModelComputeRefusal> {
        if self.state != ModelComputeLifecycle::Ready
            || self.queued_items >= self.offer.limits.maximum_queue_items
            || self
                .queued_bytes
                .checked_add(bytes)
                .is_none_or(|v| v > self.offer.limits.maximum_queue_bytes)
        {
            return Err(ModelComputeRefusal::QueueFull);
        }
        self.queued_items += 1;
        self.queued_bytes += bytes;
        Ok(())
    }
    pub fn begin(
        &mut self,
        requirement: &ModelComputeRequirement,
        queued_bytes: u64,
    ) -> Result<(), ModelComputeRefusal> {
        if self.state != ModelComputeLifecycle::Ready || self.loaded_model_identity.is_none() {
            return Err(ModelComputeRefusal::InvalidLifecycleTransition);
        }
        self.offer.admits(requirement)?;
        if self.queued_items > 0 {
            self.queued_items -= 1;
            self.queued_bytes = self.queued_bytes.saturating_sub(queued_bytes);
        }
        self.state = ModelComputeLifecycle::Active(requirement.operation);
        Ok(())
    }
    pub fn finish(&mut self) -> Result<(), ModelComputeRefusal> {
        if !matches!(self.state, ModelComputeLifecycle::Active(_)) {
            return Err(ModelComputeRefusal::InvalidLifecycleTransition);
        }
        self.state = ModelComputeLifecycle::Ready;
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<(), ModelComputeRefusal> {
        if !self.offer.limits.cancellation_supported {
            return Err(ModelComputeRefusal::CancellationUnsupported);
        }
        self.finish()
    }
    pub fn provider_lost(&mut self) {
        self.state = ModelComputeLifecycle::Lost;
        self.loaded_model_identity = None;
        self.loaded_model_bytes = 0;
        self.queued_items = 0;
        self.queued_bytes = 0;
    }
    pub fn begin_unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.transition(
            ModelComputeLifecycle::Ready,
            ModelComputeLifecycle::Unloading,
        )
    }
    pub fn shutdown(&mut self) -> Result<(), ModelComputeRefusal> {
        if self.state != ModelComputeLifecycle::Unloading {
            return Err(ModelComputeRefusal::InvalidLifecycleTransition);
        }
        self.loaded_model_identity = None;
        self.loaded_model_bytes = 0;
        self.state = ModelComputeLifecycle::Shutdown;
        Ok(())
    }
    fn transition(
        &mut self,
        from: ModelComputeLifecycle,
        to: ModelComputeLifecycle,
    ) -> Result<(), ModelComputeRefusal> {
        if self.state != from {
            return Err(ModelComputeRefusal::InvalidLifecycleTransition);
        }
        self.state = to;
        Ok(())
    }
}

impl ModelComputeRuntimeIdentity {
    fn validate(&self) -> Result<(), ModelComputeRefusal> {
        for value in [
            &self.provider_name,
            &self.runtime_name,
            &self.runtime_version,
            &self.runtime_build_identity,
            &self.adapter_artifact_identity,
            &self.device_evidence,
            &self.precision_profile,
        ] {
            text(value)?
        }
        Ok(())
    }
}
fn text(value: &str) -> Result<(), ModelComputeRefusal> {
    if value.is_empty() || value.len() > 128 {
        Err(ModelComputeRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}
fn duplicate<T: PartialEq>(values: &[T]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}
