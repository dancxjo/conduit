//! Bounded CPL3 service entry. Returned bytes and gate claims remain untrusted;
//! this backend grants no display, device or other effect authority.
use super::{domain_memory::enable_no_execute, domain_transition, service_memory::AddressSpace};
use crate::{
    protected_region::{DomainBackend, DomainCost, DomainFault, DomainRefusal, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use core::sync::atomic::{AtomicU64, Ordering};

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

pub struct ServiceDomain {
    space: AddressSpace,
    instance: u64,
    cost: DomainCost,
    quarantined: bool,
}
impl ServiceDomain {
    pub const RESERVED_BYTES: u32 = AddressSpace::RESERVED_BYTES;
    pub fn install(image: &ServiceImage<'_>) -> Result<Self, DomainRefusal> {
        let started = super::domain_ticks();
        enable_no_execute()?;
        let space = AddressSpace::install(image)?;
        let reserved_bytes = space.reserved_bytes();
        let instance = NEXT_INSTANCE
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| DomainRefusal::InvalidLifecycle)?;
        Ok(Self {
            space,
            instance,
            quarantined: false,
            cost: DomainCost {
                reserved_bytes,
                root_metadata_bytes: core::mem::size_of::<Self>() as u32,
                setup_copied_bytes: image.initialized_bytes(),
                setup_ticks: super::domain_ticks().saturating_sub(started),
                ..DomainCost::default()
            },
        })
    }
    /// Boot-local instance sequence, never a substitute for Host/Boot/grant scope.
    pub const fn instance(&self) -> u64 {
        self.instance
    }
    pub fn profile(&self) -> ServiceProfile {
        self.space.profile()
    }
    pub fn exchange(&mut self) -> Result<&mut [u8], DomainRefusal> {
        self.space.exchange()
    }
    /// Fixed private transfer window, available only between protected entries.
    /// A protocol must admit direction, length and instance before using bytes.
    pub fn transfer_window(&mut self) -> Result<&mut [u8], DomainRefusal> {
        self.space.transfer_window()
    }
    pub fn output(&self) -> Result<&[u8], DomainRefusal> {
        self.space.output()
    }
}
impl DomainBackend for ServiceDomain {
    fn enter(&mut self, maximum_work: u32) -> Result<DomainReturn, DomainRefusal> {
        // This backend admits exactly one protected entry per call. The shared
        // RTC mechanism bounds elapsed execution; it does not count semantic
        // compositor operations. The protocol separately bounds each command.
        if self.quarantined || maximum_work != 1 {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let budget = match self.profile() {
            ServiceProfile::Compositor => super::domain_budget::EntryBudget::Ordinary,
            ServiceProfile::Draft | ServiceProfile::Home => {
                super::domain_budget::EntryBudget::Draft
            }
        };
        let result = domain_transition::enter_context_with_budget(self.space.context()?, budget)?;
        self.cost.entries += 1;
        self.cost.address_space_switches += 2;
        self.cost.tlb_flushes += 2;
        self.cost.scheduler_returns += 1;
        let (budget, source) = super::domain_budget::user_interrupts();
        let interrupts = u64::from(budget) + u64::from(source);
        self.cost.interrupt_entries += interrupts;
        self.cost.source_timer_interrupts += u64::from(source);
        self.cost.privilege_transitions +=
            2 + 2 * interrupts.saturating_sub(u64::from(result.origin == 2));
        let returned = match (result.origin, result.value) {
            (0, 0) => {
                self.cost.gate_transitions += 1;
                DomainReturn::Yielded
            }
            (0, 0x200) => {
                self.cost.gate_transitions += 1;
                DomainReturn::Gate
            }
            (2, 4) => {
                self.cost.preemptions += 1;
                DomainReturn::Fault(DomainFault::WorkExhausted)
            }
            (1, 0x10d) => DomainReturn::Fault(DomainFault::PrivilegedOperation),
            (1, 0x10e) => DomainReturn::Fault(DomainFault::Memory),
            (1, _) => DomainReturn::Fault(DomainFault::InvalidInstruction),
            _ => DomainReturn::Fault(DomainFault::InvalidGate),
        };
        // A fault cannot leave an executable instance or readable stale response.
        if matches!(returned, DomainReturn::Fault(_)) {
            self.quarantine();
        }
        Ok(returned)
    }
    fn quarantine(&mut self) {
        if !self.quarantined {
            let started = super::domain_ticks();
            self.cost.teardown_zeroed_bytes = self.space.quarantine();
            self.cost.teardown_ticks = super::domain_ticks().saturating_sub(started);
            self.quarantined = true;
        }
    }
    fn cost(&self) -> DomainCost {
        self.cost
    }
}
