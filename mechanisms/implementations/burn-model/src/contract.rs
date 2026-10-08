//! Small framework-independent declaration used by the hosted Rust realization.
use conduit_ai::{ModelComputeLimits, ModelSignature};
use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGroup {
    pub identity: String,
    pub trainable: bool,
}

/// Includes candidate parameters/optimizer storage, not merely inference weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceEstimate {
    pub model_bytes: u64,
    pub optimizer_bytes: u64,
    pub candidate_bytes: u64,
    pub temporary_bytes: u64,
    pub checkpoint_bytes: u64,
    pub maximum_work_per_step: u64,
}

impl ResourceEstimate {
    pub fn working_bytes(self) -> Result<u64, Error> {
        self.optimizer_bytes
            .checked_add(self.candidate_bytes)
            .and_then(|n| n.checked_add(self.temporary_bytes))
            .ok_or(Error::ResourceBound)
    }
}

#[derive(Debug, Clone)]
pub struct AuthoringDescriptor {
    pub architecture: String,
    pub config_identity: [u8; 32],
    pub checkpoint_schema: u32,
    /// Exact host-local realization profiles; these never enter semantic plots.
    pub supported_profiles: Vec<String>,
    pub signature: ModelSignature,
    pub groups: Vec<ParameterGroup>,
    pub resources: ResourceEstimate,
    pub limits: ModelComputeLimits,
}

impl AuthoringDescriptor {
    pub fn validate(&self) -> Result<(), Error> {
        if self.architecture.is_empty()
            || self.architecture.len() > 128
            || self.config_identity == [0; 32]
            || self.checkpoint_schema == 0
            || self.supported_profiles.is_empty()
            || self.supported_profiles.len() > 16
            || self
                .supported_profiles
                .iter()
                .any(|p| p.is_empty() || p.len() > 128)
            || self
                .supported_profiles
                .iter()
                .enumerate()
                .any(|(i, p)| self.supported_profiles[..i].contains(p))
            || self.groups.is_empty()
            || self.groups.len() > 32
        {
            return Err(Error::InvalidDescriptor);
        }
        self.signature
            .validate()
            .map_err(|_| Error::InvalidDescriptor)?;
        for (i, group) in self.groups.iter().enumerate() {
            if group.identity.is_empty()
                || group.identity.len() > 128
                || self.groups[..i]
                    .iter()
                    .any(|prior| prior.identity == group.identity)
            {
                return Err(Error::InvalidDescriptor);
            }
        }
        for port in self
            .signature
            .inputs
            .get()
            .iter()
            .chain(self.signature.outputs.get().iter())
        {
            let conduit_ai::ModelValueConstraint::Tensor(tensor) = &port.value else {
                return Err(Error::InvalidDescriptor);
            };
            if tensor.constraint().elements.get().as_slice() != [conduit_data::TensorElement::F32] {
                return Err(Error::InvalidDescriptor);
            }
            if port.presence != conduit_ai::ModelPortPresence::Required {
                return Err(Error::InvalidDescriptor);
            }
        }
        let r = self.resources;
        if r.model_bytes == 0
            || r.optimizer_bytes == 0
            || r.candidate_bytes < r.model_bytes
            || r.temporary_bytes == 0
            || r.checkpoint_bytes == 0
            || r.maximum_work_per_step == 0
            || r.model_bytes > self.limits.maximum_model_bytes
            || r.working_bytes()? > self.limits.maximum_working_memory_bytes
        {
            return Err(Error::ResourceBound);
        }
        Ok(())
    }
}
