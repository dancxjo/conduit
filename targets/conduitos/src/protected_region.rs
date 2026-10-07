//! Root-owned realization beneath an admitted execution region.
//!
//! Backend construction must install processor enforcement before returning a
//! domain. This interface does not turn cooperative native execution into
//! confinement, and a backend without enforcement must return Unsupported.

use conduit_core::{ActivePlayIdentity, ExecutionRegionId, Plan, bind_active_play, verify_plan};

use crate::protection_domain::{KernelCapabilityTable, KernelRevocationCause, ProtectionDomainId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionBinding {
    pub active: ActivePlayIdentity,
    pub region: ExecutionRegionId,
    pub domain: ProtectionDomainId,
}

impl RegionBinding {
    pub fn admit(
        plan: &Plan,
        active: &ActivePlayIdentity,
        region: &ExecutionRegionId,
        domain: ProtectionDomainId,
    ) -> Result<Self, DomainRefusal> {
        if domain.0 == 0
            || !verify_plan(plan)
            || active.plan_id != plan.plan_id
            || bind_active_play(
                &plan.plan_id,
                &active.host_id,
                &active.boot_id,
                active.play_sequence,
            ) != *active
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let mut regions = plan
            .fragments
            .iter()
            .filter(|fragment| {
                fragment.host_id == active.host_id && fragment.boot_id == active.boot_id
            })
            .flat_map(|fragment| &fragment.execution_regions)
            .filter(|candidate| &candidate.region_id == region);
        let admitted = regions.next().ok_or(DomainRefusal::WrongBinding)?;
        if regions.next().is_some() || admitted.admitted_placements.is_empty() {
            return Err(DomainRefusal::WrongBinding);
        }
        Ok(Self {
            active: active.clone(),
            region: region.clone(),
            domain,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainRefusal {
    Unsupported,
    WrongBinding,
    InvalidMemory,
    InvalidLifecycle,
    WorkExhausted,
    BackendFailure,
}
impl DomainRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "protected-execution-unsupported",
            Self::WrongBinding => "protected-domain-binding-refused",
            Self::InvalidMemory => "protected-domain-memory-refused",
            Self::InvalidLifecycle => "protected-domain-lifecycle-refused",
            Self::WorkExhausted => "protected-domain-work-exhausted",
            Self::BackendFailure => "protected-domain-backend-failed",
        }
    }
}

/// Mapping purpose is realization truth. Physical/kernel pointers never occur
/// in the portable request; the backend owns storage and translation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPurpose {
    Executable,
    PrivateState,
    Stack,
    SharedInput,
    SharedOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainMemory {
    pub executable_bytes: u32,
    pub private_bytes: u32,
    pub stack_bytes: u32,
    pub input_bytes: u32,
    pub output_bytes: u32,
}

impl DomainMemory {
    pub fn total(self) -> Result<u32, DomainRefusal> {
        if self.executable_bytes == 0 || self.stack_bytes == 0 {
            return Err(DomainRefusal::InvalidMemory);
        }
        [
            self.executable_bytes,
            self.private_bytes,
            self.stack_bytes,
            self.input_bytes,
            self.output_bytes,
        ]
        .into_iter()
        .try_fold(0u32, |sum, bytes| sum.checked_add(bytes))
        .ok_or(DomainRefusal::InvalidMemory)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DomainCost {
    pub entries: u64,
    pub gate_transitions: u64,
    pub copied_bytes: u64,
    pub address_space_switches: u64,
    pub scheduler_returns: u64,
    pub preemptions: u64,
    pub reserved_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainFault {
    Memory,
    PrivilegedOperation,
    InvalidInstruction,
    InvalidGate,
    WorkExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainReturn {
    /// Only an effect/input boundary returns to Root. Local semantics stay in
    /// the domain between such boundaries.
    Gate,
    Yielded,
    Preempted,
    Completed,
    Fault(DomainFault),
}

/// Architecture-specific machinery implements this contract. A finite work
/// grant must be enforced independently of implementation cooperation.
pub trait DomainBackend {
    fn enter(&mut self, maximum_work: u32) -> Result<DomainReturn, DomainRefusal>;
    fn quarantine(&mut self);
    fn cost(&self) -> DomainCost;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainState {
    Ready,
    Suspended,
    Completed,
    Revoked(KernelRevocationCause),
    Faulted(DomainFault),
}

pub struct ProtectedRegion<B: DomainBackend> {
    binding: RegionBinding,
    backend: B,
    state: DomainState,
}

impl<B: DomainBackend> ProtectedRegion<B> {
    /// Called only after the backend installs the admitted code/state/stack
    /// and explicit bounded windows with processor-enforced permissions.
    pub fn installed(binding: RegionBinding, backend: B) -> Self {
        Self {
            binding,
            backend,
            state: DomainState::Ready,
        }
    }

    pub fn binding(&self) -> &RegionBinding {
        &self.binding
    }
    pub fn state(&self) -> DomainState {
        self.state
    }
    pub fn cost(&self) -> DomainCost {
        self.backend.cost()
    }
    pub fn backend_mut(&mut self) -> Result<&mut B, DomainRefusal> {
        if !matches!(self.state, DomainState::Ready | DomainState::Suspended) {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        Ok(&mut self.backend)
    }

    pub fn resume(
        &mut self,
        current: &RegionBinding,
        maximum_work: u32,
        capabilities: &mut KernelCapabilityTable,
    ) -> Result<DomainReturn, DomainRefusal> {
        if current != &self.binding {
            return Err(DomainRefusal::WrongBinding);
        }
        if !matches!(self.state, DomainState::Ready | DomainState::Suspended) {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        if maximum_work == 0 {
            return Err(DomainRefusal::WorkExhausted);
        }
        let outcome = match self.backend.enter(maximum_work) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.revoke(KernelRevocationCause::ProviderLost, capabilities);
                return Err(error);
            }
        };
        match outcome {
            DomainReturn::Completed => {
                self.revoke(KernelRevocationCause::PlayCompleted, capabilities);
                self.state = DomainState::Completed;
            }
            DomainReturn::Fault(fault) => {
                self.revoke(KernelRevocationCause::ProtectionFault, capabilities);
                self.state = DomainState::Faulted(fault);
            }
            _ => self.state = DomainState::Suspended,
        }
        Ok(outcome)
    }

    pub fn revoke(
        &mut self,
        cause: KernelRevocationCause,
        capabilities: &mut KernelCapabilityTable,
    ) {
        if !matches!(self.state, DomainState::Ready | DomainState::Suspended) {
            return;
        }
        capabilities.revoke_domain(self.binding.domain, cause);
        self.backend.quarantine();
        self.state = DomainState::Revoked(cause);
    }
}

#[cfg(test)]
mod tests;
