//! Exact declared elapsed-second to clock-tick preparation. No calibration claim.
use crate::generated::*;
use crate::source_execution::{AudioSourceExecution, AudioSourceExecutionRefusal, Program};
use crate::source_programs::{CLOCK_BINDING_VALID, CLOCK_PROJECT, CLOCK_QUERY_ELIGIBLE};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum AudioClockProjectionRefusal {
    Admission(NativeBindingRefusal),
    Source(AudioSourceExecutionRefusal),
    UnsupportedBinding,
    ForeignOrUnsupportedQuery,
    InvalidOwnerRelation,
}
impl From<NativeBindingRefusal> for AudioClockProjectionRefusal {
    fn from(value: NativeBindingRefusal) -> Self {
        Self::Admission(value)
    }
}
impl From<AudioSourceExecutionRefusal> for AudioClockProjectionRefusal {
    fn from(value: AudioSourceExecutionRefusal) -> Self {
        Self::Source(value)
    }
}

/// Every original carrier and executed numerical program remains inspectable.
pub struct AudioClockProjectionReceipt {
    binding_canonical: Vec<u8>,
    query_canonical: Vec<u8>,
    binding_executions: Vec<AudioSourceExecution>,
    executions: Vec<AudioSourceExecution>,
    result: AudioClockProjectionResult,
    admitted_canonical: Vec<u8>,
}
impl AudioClockProjectionReceipt {
    pub fn binding(&self) -> &AudioClockBinding {
        self.result.eligible().request().binding()
    }
    pub fn binding_canonical(&self) -> &[u8] {
        &self.binding_canonical
    }
    pub fn query(&self) -> &AudioTrajectoryQuery {
        self.result.eligible().request().query()
    }
    pub fn query_canonical(&self) -> &[u8] {
        &self.query_canonical
    }
    pub fn binding_executions(&self) -> &[AudioSourceExecution] {
        &self.binding_executions
    }
    pub fn executions(&self) -> &[AudioSourceExecution] {
        &self.executions
    }
    pub fn result(&self) -> &AudioClockProjectionResult {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted_canonical
    }
}

/// Allocating finite semantic preparation; it does not offer a running clock.
pub struct PreparedAudioClockBinding {
    binding: AudioClockBinding,
    binding_canonical: Vec<u8>,
    binding_executions: Vec<AudioSourceExecution>,
    eligible: Program,
    projection: Program,
}
impl PreparedAudioClockBinding {
    pub fn new(canonical: &[u8]) -> Result<Self, AudioClockProjectionRefusal> {
        let binding = AudioClockBinding::decode(canonical)?;
        binding
            .relation()
            .validate()
            .map_err(|_| AudioClockProjectionRefusal::InvalidOwnerRelation)?;
        let mut binding_executions = Vec::new();
        if !Program::new(CLOCK_BINDING_VALID)?.boolean(binding.clone(), &mut binding_executions)? {
            return Err(AudioClockProjectionRefusal::UnsupportedBinding);
        }
        Ok(Self {
            binding,
            binding_canonical: canonical.to_vec(),
            binding_executions,
            eligible: Program::new(CLOCK_QUERY_ELIGIBLE)?,
            projection: Program::new(CLOCK_PROJECT)?,
        })
    }
    pub fn binding(&self) -> &AudioClockBinding {
        &self.binding
    }
    pub fn binding_canonical(&self) -> &[u8] {
        &self.binding_canonical
    }
    pub fn project(
        &self,
        query_canonical: &[u8],
    ) -> Result<AudioClockProjectionReceipt, AudioClockProjectionRefusal> {
        let query = AudioTrajectoryQuery::decode(query_canonical)?;
        let request = AudioClockProjectionRequest::new(self.binding.clone(), query)?;
        let mut executions = Vec::new();
        if !self.eligible.boolean(request.clone(), &mut executions)? {
            return Err(AudioClockProjectionRefusal::ForeignOrUnsupportedQuery);
        }
        let eligible = AudioClockProjectionEligible::new(request)?;
        let raw: AudioClockProjectionRaw =
            self.projection.native(eligible.clone(), &mut executions)?;
        let result = AudioClockProjectionResult::new(eligible, raw)?;
        let admitted_canonical = result.encode()?;
        Ok(AudioClockProjectionReceipt {
            binding_canonical: self.binding_canonical.clone(),
            query_canonical: query_canonical.to_vec(),
            binding_executions: self.binding_executions.clone(),
            executions,
            result,
            admitted_canonical,
        })
    }
}
