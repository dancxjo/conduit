//! Bounded CPL3 service entry. Returned bytes and gate claims remain untrusted;
//! this backend grants no display, device or other effect authority.
use super::{domain_memory::enable_no_execute, domain_transition, service_memory::AddressSpace};
use crate::{
    protected_region::{DomainBackend, DomainCost, DomainRefusal, DomainReturn},
    service_memory::{ServiceImage, ServiceProfile},
};
use core::sync::atomic::{AtomicU64, Ordering};

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

pub struct ServiceDomain {
    pub(super) space: AddressSpace,
    instance: u64,
    cost: DomainCost,
    pub(super) quarantined: bool,
    #[cfg(feature = "ordinary-domain-proof")]
    pub(super) clock_binding: Option<super::clock_boundary::Binding>,
}
/// Immutable actual native entry observation; never guest-declared work or
/// Source fuel. The Application owner must enforce its cumulative ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationEntryReceipt(super::domain_budget::NativeEntryReceipt);
impl ApplicationEntryReceipt {
    pub const fn instance(self) -> u64 {
        self.0.instance()
    }
    pub const fn sequence(self) -> u64 {
        self.0.sequence()
    }
    pub const fn admitted_rtc(self) -> u32 {
        self.0.admitted()
    }
    pub const fn consumed_rtc(self) -> u32 {
        self.0.rtc()
    }
    pub const fn source_interrupts(self) -> u32 {
        self.0.source()
    }
    pub const fn result(self) -> DomainReturn {
        self.0.outcome()
    }
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
            #[cfg(feature = "ordinary-domain-proof")]
            clock_binding: None,
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
impl ServiceDomain {
    /// Lower this Application entry to the logical operation's remaining
    /// existing64 RTC ticks. This method does not replenish that operation.
    pub fn enter_application_remaining(
        &mut self,
        remaining: u32,
    ) -> Result<ApplicationEntryReceipt, DomainRefusal> {
        // Invalid caller arguments are refused before attempting native entry;
        // they must not revoke an unrelated service profile or valid instance.
        let cap = super::domain_budget::ApplicationRemaining::new(self.profile(), remaining)?;
        let result = self.enter_budget(super::domain_budget::EntryBudget::Application(cap));
        result.map(ApplicationEntryReceipt)
    }
    fn enter_budget(
        &mut self,
        budget: super::domain_budget::EntryBudget,
    ) -> Result<super::domain_budget::NativeEntryReceipt, DomainRefusal> {
        if self.quarantined {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let identity = super::domain_budget::EntryIdentity::next(self.instance, self.cost.entries)?;
        let sequence = identity.sequence();
        #[cfg(feature = "ordinary-domain-proof")]
        super::irq_envelope::validate_entry(self.instance)?;
        let context = self.space.context()?;
        #[cfg(feature = "ordinary-domain-proof")]
        let permit = self
            .clock_binding
            .as_mut()
            .and_then(|binding| binding.take(self.instance, &context));
        // From this call onward entry has been attempted. Any refusal or
        // missing/invalid observation is terminal for every service profile.
        let result = match domain_transition::enter_context_with_receipt(
            context,
            budget,
            identity,
            #[cfg(feature = "ordinary-domain-proof")]
            permit,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.quarantine();
                return Err(error);
            }
        };
        if let Err(error) = result.validate(self.instance, sequence) {
            self.quarantine();
            return Err(error);
        }
        self.cost.entries = sequence;
        self.cost.address_space_switches += 2;
        self.cost.tlb_flushes += 2;
        self.cost.scheduler_returns += 1;
        let (budget, source) = (result.rtc(), result.source());
        let interrupts = u64::from(budget) + u64::from(source);
        self.cost.interrupt_entries += interrupts;
        self.cost.source_timer_interrupts += u64::from(source);
        self.cost.privilege_transitions +=
            2 + 2 * interrupts.saturating_sub(u64::from(result.origin() == 2));
        let returned = result.outcome();
        result.account_outcome(&mut self.cost);
        // A fault cannot leave an executable instance or readable stale response.
        if matches!(returned, DomainReturn::Fault(_)) {
            self.quarantine();
            if let Some(pc) = result.fault_pc() {
                // Revocation and zeroing have completed. Fixed serial fields only;
                // no IRQ output, allocation, guest response or admission decision.
                super::serial::early_write(b"CONDUIT_SERVICE_BUDGET_PC instance=");
                super::serial::write_decimal(result.instance());
                super::serial::early_write(b" sequence=");
                super::serial::write_decimal(result.sequence());
                super::serial::early_write(b" admitted=");
                super::serial::write_decimal(u64::from(result.admitted()));
                super::serial::early_write(b" rtc=");
                super::serial::write_decimal(u64::from(result.rtc()));
                super::serial::early_write(b" source=");
                super::serial::write_decimal(u64::from(result.source()));
                super::serial::early_write(b" rip=");
                super::serial::write_decimal(pc.rip);
                super::serial::early_write(b" rsp=");
                super::serial::write_decimal(pc.rsp);
                super::serial::early_write(b"\n");
            }
        }
        Ok(result)
    }
}
impl DomainBackend for ServiceDomain {
    fn enter(&mut self, maximum_work: u32) -> Result<DomainReturn, DomainRefusal> {
        if self.quarantined || maximum_work != 1 {
            return Err(DomainRefusal::InvalidLifecycle);
        }
        let budget = match self.profile() {
            ServiceProfile::Compositor => super::domain_budget::EntryBudget::Ordinary,
            ServiceProfile::Draft
            | ServiceProfile::Home
            | ServiceProfile::Runtime
            | ServiceProfile::Application
            | ServiceProfile::Network
            | ServiceProfile::OrdinaryRuntime
            | ServiceProfile::Mask
            | ServiceProfile::Tls => super::domain_budget::EntryBudget::Draft,
        };
        self.enter_budget(budget).map(|receipt| receipt.outcome())
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
