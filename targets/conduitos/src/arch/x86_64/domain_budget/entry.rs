//! Root-private entry policy and immutable native observations, not Source fuel.
use crate::{
    protected_region::{DomainCost, DomainFault, DomainRefusal, DomainReturn},
    service_memory::ServiceProfile,
};
// Existing ordinary frame-sized bound; no grant change in this interface.
pub(in crate::arch::x86_64) const ORDINARY_TICKS: u32 = 16;
pub(in crate::arch::x86_64) const DRAFT_TICKS: u32 = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::arch::x86_64) struct ApplicationRemaining(u32);
impl ApplicationRemaining {
    pub(in crate::arch::x86_64) fn new(
        profile: ServiceProfile,
        ticks: u32,
    ) -> Result<Self, DomainRefusal> {
        if profile != ServiceProfile::Application || !(1..=DRAFT_TICKS).contains(&ticks) {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        Ok(Self(ticks))
    }
}
/// Existing defaults remain unchanged. Application can only lower Draft's cap.
#[derive(Clone, Copy)]
pub(in crate::arch::x86_64) enum EntryBudget {
    Ordinary,
    Draft,
    Application(ApplicationRemaining),
}
impl EntryBudget {
    pub(in crate::arch::x86_64) const fn ticks(self) -> u32 {
        match self {
            Self::Ordinary => ORDINARY_TICKS,
            Self::Draft => DRAFT_TICKS,
            Self::Application(cap) => cap.0,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::arch::x86_64) struct EntryIdentity {
    instance: u64,
    sequence: u64,
}
impl EntryIdentity {
    pub(in crate::arch::x86_64) fn next(
        instance: u64,
        previous: u64,
    ) -> Result<Self, DomainRefusal> {
        Self::new(
            instance,
            previous
                .checked_add(1)
                .ok_or(DomainRefusal::InvalidLifecycle)?,
        )
    }
    pub(in crate::arch::x86_64) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(in crate::arch::x86_64) fn new(
        instance: u64,
        sequence: u64,
    ) -> Result<Self, DomainRefusal> {
        if instance == 0 || sequence == 0 {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        Ok(Self { instance, sequence })
    }
}
/// Created only from the live native guard and actual assembly return. No
/// mutable global is read when this value is later consumed by ServiceDomain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::arch::x86_64) struct NativeEntryReceipt {
    identity: EntryIdentity,
    admitted: u32,
    rtc: u32,
    source: u32,
    value: u64,
    origin: u64,
    fault_pc: Option<super::fault_pc::FaultPc>,
}
impl NativeEntryReceipt {
    pub(super) fn capture(
        identity: EntryIdentity,
        budget: EntryBudget,
        counts: (u32, u32),
        value: u64,
        origin: u64,
    ) -> Result<Self, DomainRefusal> {
        if counts.0 > budget.ticks() {
            return Err(DomainRefusal::BackendFailure);
        }
        Ok(Self {
            identity,
            admitted: budget.ticks(),
            rtc: counts.0,
            source: counts.1,
            value,
            origin,
            fault_pc: None,
        })
    }
    pub(super) fn with_fault_pc(mut self, pc: Option<super::fault_pc::FaultPc>) -> Self {
        // Optional diagnostics cannot alter or rescue any return disposition.
        if self.origin == 2 && self.value == 4 {
            self.fault_pc = pc;
        }
        self
    }
    pub(in crate::arch::x86_64) const fn fault_pc(self) -> Option<super::fault_pc::FaultPc> {
        self.fault_pc
    }
    pub(in crate::arch::x86_64) fn validate(
        &self,
        instance: u64,
        sequence: u64,
    ) -> Result<(), DomainRefusal> {
        if self.identity != (EntryIdentity { instance, sequence }) {
            return Err(DomainRefusal::WrongBinding);
        }
        Ok(())
    }
    pub(in crate::arch::x86_64) const fn instance(self) -> u64 {
        self.identity.instance
    }
    pub(in crate::arch::x86_64) const fn sequence(self) -> u64 {
        self.identity.sequence
    }
    pub(in crate::arch::x86_64) const fn admitted(self) -> u32 {
        self.admitted
    }
    pub(in crate::arch::x86_64) const fn rtc(self) -> u32 {
        self.rtc
    }
    pub(in crate::arch::x86_64) const fn source(self) -> u32 {
        self.source
    }
    pub(in crate::arch::x86_64) const fn origin(self) -> u64 {
        self.origin
    }
    pub(in crate::arch::x86_64) const fn outcome(self) -> DomainReturn {
        match (self.origin, self.value) {
            (0, 0) => DomainReturn::Yielded,
            (0, 0x200) => DomainReturn::Gate,
            (2, 4) => DomainReturn::Fault(DomainFault::WorkExhausted),
            (1, 0x10d) => DomainReturn::Fault(DomainFault::PrivilegedOperation),
            (1, 0x10e) => DomainReturn::Fault(DomainFault::Memory),
            (1, _) => DomainReturn::Fault(DomainFault::InvalidInstruction),
            _ => DomainReturn::Fault(DomainFault::InvalidGate),
        }
    }
    /// Preserve the existing shared service-entry outcome counters for every
    /// profile; immutable receipt capture does not replace this accounting.
    pub(in crate::arch::x86_64) fn account_outcome(self, cost: &mut DomainCost) {
        match self.outcome() {
            DomainReturn::Yielded | DomainReturn::Gate => cost.gate_transitions += 1,
            DomainReturn::Fault(DomainFault::WorkExhausted) => cost.preemptions += 1,
            DomainReturn::Fault(_) | DomainReturn::Preempted | DomainReturn::Completed => {}
        }
    }
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
