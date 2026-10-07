//! Root preparation and lifecycle for the ordinary text region's implementation.
use crate::{
    arch::TextDomain,
    composition::MachineRunError,
    protected_region::{DomainBinding, DomainReturn, ProtectedRegion, RegionBinding},
    protection_domain::{KernelCapabilityTable, KernelRevocationCause, ProtectionDomainId},
    text_upper::{MAXIMUM_BYTES, UppercaseText},
};
use conduit_core::{ActivePlayIdentity, Plan};
use core::sync::atomic::{AtomicU32, Ordering};

mod chain;
pub(crate) use chain::{KeyboardChainError, PureKeyboardOutput};
mod body;
pub(crate) use body::BodyTextAdmission;

static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(1);
pub(crate) const ROOT_METADATA_CEILING: u32 = {
    let text = core::mem::size_of::<crate::text_planned_kernel::TextPlannedKernel>();
    let dual = core::mem::size_of::<crate::dual_region_kernel::DualRegionKernel>();
    if text > dual { text } else { dual }
} as u32
    + 2 * (4 * 64 + 64);

pub(crate) trait TextOwner: DomainBinding {
    fn scope(
        &self,
        serial: &crate::domain_serial_scope::SerialScope,
        generation: Option<u64>,
    ) -> Result<
        crate::protection_domain::KernelCapabilityScope,
        crate::protected_region::DomainRefusal,
    >;
    fn region_id(&self) -> &str;
    fn plan_id(&self) -> &str;
    fn play_id(&self) -> &str;
}

impl TextOwner for RegionBinding {
    fn scope(
        &self,
        serial: &crate::domain_serial_scope::SerialScope,
        generation: Option<u64>,
    ) -> Result<
        crate::protection_domain::KernelCapabilityScope,
        crate::protected_region::DomainRefusal,
    > {
        serial.current(self, generation)
    }
    fn region_id(&self) -> &str {
        self.region.as_str()
    }
    fn plan_id(&self) -> &str {
        self.active.plan_id.as_str()
    }
    fn play_id(&self) -> &str {
        self.active.active_play_id.as_str()
    }
}
impl TextOwner for crate::protected_region::BodyRegionBinding {
    fn scope(
        &self,
        serial: &crate::domain_serial_scope::SerialScope,
        generation: Option<u64>,
    ) -> Result<
        crate::protection_domain::KernelCapabilityScope,
        crate::protected_region::DomainRefusal,
    > {
        serial.current_body(self, generation)
    }
    fn region_id(&self) -> &str {
        self.region.as_str()
    }
    fn plan_id(&self) -> &str {
        self.active.plan_id.as_str()
    }
    fn play_id(&self) -> &str {
        self.active.active_play_id.as_str()
    }
}

pub(crate) struct ProtectedText<I: TextOwner = RegionBinding> {
    region: ProtectedRegion<TextDomain, I>,
    capabilities: KernelCapabilityTable,
    current: I,
    serial: crate::domain_serial_scope::SerialScope,
    serial_handle: crate::protection_domain::KernelCapabilityHandle,
    diagnostic_fixture: bool,
}

impl ProtectedText {
    pub fn prepare(
        plan: &Plan,
        active: &ActivePlayIdentity,
        fixed: &crate::offer::HostOffer<'_>,
    ) -> Result<Self, MachineRunError> {
        Self::prepare_with_kernel_bytes(
            plan,
            active,
            fixed,
            core::mem::size_of::<crate::text_planned_kernel::TextPlannedKernel>(),
        )
    }

    pub(crate) fn prepare_with_kernel_bytes(
        plan: &Plan,
        active: &ActivePlayIdentity,
        fixed: &crate::offer::HostOffer<'_>,
        kernel_bytes: usize,
    ) -> Result<Self, MachineRunError> {
        let started = TextDomain::ticks();
        let domain = NEXT_DOMAIN
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| MachineRunError::KernelConstruction)?;
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id == active.host_id && fragment.boot_id == active.boot_id
            })
            .ok_or(MachineRunError::KernelConstruction)?;
        let mut regions = fragment.execution_regions.iter().filter(|region| {
            region.execution_profile_id.as_str() == crate::ordinary_plan::PROTECTED_REGION_PROFILE
        });
        let region = regions.next().ok_or(MachineRunError::KernelConstruction)?;
        if regions.next().is_some() {
            return Err(MachineRunError::KernelConstruction);
        }
        if region.execution_profile_id.as_str() != crate::ordinary_plan::PROTECTED_REGION_PROFILE
            || !region.preemption_required
            || !region.isolation_required
            || region.requirements.runtime_memory_bytes < TextDomain::RESERVED_BYTES
        {
            return Err(MachineRunError::KernelConstruction);
        }
        let binding =
            RegionBinding::admit(plan, active, &region.region_id, ProtectionDomainId(domain))
                .map_err(MachineRunError::ProtectionDomain)?;
        let backend = TextDomain::install().map_err(MachineRunError::ProtectionDomain)?;
        let serial = crate::domain_serial_scope::SerialScope::admit(plan, &binding, fixed)
            .map_err(MachineRunError::ProtectionDomain)?;
        let mut capabilities = capability_table(fixed.generation)?;
        let serial_handle = capabilities
            .issue(binding.domain, serial.scope)
            .map_err(MachineRunError::ProtectionCapability)?;
        let mut protected = Self {
            current: binding.clone(),
            region: ProtectedRegion::installed(binding, backend),
            capabilities,
            serial,
            serial_handle,
            diagnostic_fixture: false,
        };
        let heap_bytes = [&protected.current, protected.region.binding()]
            .into_iter()
            .map(|binding| {
                binding.active.host_id.0.capacity()
                    + binding.active.boot_id.0.capacity()
                    + binding.active.plan_id.0.capacity()
                    + binding.active.active_play_id.0.capacity()
                    + binding.region.0.capacity()
            })
            .sum::<usize>();
        let root_metadata_bytes = u32::try_from(kernel_bytes + heap_bytes)
            .map_err(|_| MachineRunError::KernelConstruction)?;
        if TextDomain::RESERVED_BYTES
            .checked_add(root_metadata_bytes)
            .is_none_or(|bytes| bytes > region.requirements.runtime_memory_bytes)
        {
            return Err(MachineRunError::ProtectionDomain(
                crate::protected_region::DomainRefusal::InvalidMemory,
            ));
        }
        protected
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .preparation_cost(started, root_metadata_bytes);
        Ok(protected)
    }
}

impl<I: TextOwner> ProtectedText<I> {
    pub fn reset_keymap(&mut self) -> Result<(), MachineRunError> {
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .initialize_keymap()
            .map_err(MachineRunError::ProtectionDomain)?;
        self.return_from_pure()
    }

    fn return_from_pure(&mut self) -> Result<(), MachineRunError> {
        match self
            .region
            .resume(&self.current, 1, &mut self.capabilities)
            .map_err(MachineRunError::ProtectionDomain)?
        {
            DomainReturn::Yielded => Ok(()),
            DomainReturn::Fault(fault) => Err(MachineRunError::ProtectionFault(fault)),
            _ => {
                self.region.fault(
                    crate::protected_region::DomainFault::InvalidGate,
                    &mut self.capabilities,
                );
                Err(MachineRunError::ProtectionFault(
                    crate::protected_region::DomainFault::InvalidGate,
                ))
            }
        }
    }

    pub fn uppercase(&mut self, input: &[u8]) -> Result<UppercaseText, MachineRunError> {
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .input(input)
            .map_err(MachineRunError::ProtectionDomain)?;
        match self
            .region
            .resume(&self.current, 1, &mut self.capabilities)
            .map_err(MachineRunError::ProtectionDomain)?
        {
            DomainReturn::Yielded => {}
            DomainReturn::Fault(fault) => return Err(MachineRunError::ProtectionFault(fault)),
            _ => {
                self.region.fault(
                    crate::protected_region::DomainFault::InvalidGate,
                    &mut self.capabilities,
                );
                return Err(MachineRunError::ProtectionFault(
                    crate::protected_region::DomainFault::InvalidGate,
                ));
            }
        }
        let backend = self
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?;
        match backend.status() {
            1 => return Err(MachineRunError::TextMalformedUtf8),
            2 => return Err(MachineRunError::TextOutputOverflow),
            0 => {}
            _ => return Err(MachineRunError::KernelFailure),
        }
        let mut output = UppercaseText {
            bytes: [0; MAXIMUM_BYTES],
            len: 0,
        };
        output.len = backend
            .output(&mut output.bytes)
            .map_err(MachineRunError::ProtectionDomain)?;
        if core::str::from_utf8(output.as_bytes()).is_err() {
            self.region.fault(
                crate::protected_region::DomainFault::InvalidGate,
                &mut self.capabilities,
            );
            return Err(MachineRunError::ProtectionFault(
                crate::protected_region::DomainFault::InvalidGate,
            ));
        }
        Ok(output)
    }

    pub fn revoke(&mut self, cause: KernelRevocationCause) {
        self.region.revoke(cause, &mut self.capabilities);
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn mark_fixture(&mut self) {
        self.diagnostic_fixture = true;
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn gate_probe(&mut self, probe: u32, target: u64) {
        self.region
            .backend_mut()
            .expect("live diagnostic domain")
            .configure_gate_probe(probe, target);
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn handle_for_probe(&self) -> u64 {
        self.serial_handle.raw_for_domain()
    }
    #[cfg(feature = "ordinary-domain-proof")]
    pub(crate) fn state_for_probe(&self) -> crate::protected_region::DomainState {
        self.region.state()
    }

    pub fn present(
        &mut self,
        input: &[u8],
        serial: &mut impl crate::machine::SerialBase,
    ) -> Result<(), MachineRunError> {
        use crate::protected_region::DomainFault;
        let current = self
            .current
            .scope(&self.serial, serial.provider_generation())
            .map_err(|refusal| {
                self.revoke(KernelRevocationCause::BaseReplaced);
                MachineRunError::ProtectionDomain(refusal)
            })?;
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .presentation(input, self.serial_handle.raw_for_domain())
            .map_err(MachineRunError::ProtectionDomain)?;
        match self
            .region
            .resume(&self.current, 1, &mut self.capabilities)
            .map_err(MachineRunError::ProtectionDomain)?
        {
            DomainReturn::Gate => {}
            DomainReturn::Fault(fault) => return Err(MachineRunError::ProtectionFault(fault)),
            _ => {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                return Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate));
            }
        }
        let mut copied = [0; MAXIMUM_BYTES];
        let (raw, operation, work_units, length) = self
            .region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .effect_request(&mut copied)
            .map_err(|error| {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                MachineRunError::ProtectionDomain(error)
            })?;
        if work_units == 0 || core::str::from_utf8(&copied[..length]).is_err() {
            self.region
                .fault(DomainFault::InvalidGate, &mut self.capabilities);
            return Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate));
        }
        let claim = crate::protection_domain::KernelOperationClaim {
            boot: current.boot,
            plan: current.plan,
            play: current.play,
            base_generation: current.base_generation,
            resource_generation: current.resource_generation,
            operation,
            parameter_bytes: length as u32,
            work_units,
        };
        let lease = self
            .capabilities
            .authorize_current(
                self.current.domain(),
                crate::protection_domain::KernelCapabilityHandle::from_untrusted(raw),
                &current,
                claim,
            )
            .map_err(|error| {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                MachineRunError::ProtectionCapability(error)
            })?;
        if serial.present(&copied[..length]).is_err() {
            self.revoke(KernelRevocationCause::ProviderLost);
            return Err(MachineRunError::SerialBaseFailure);
        }
        if self
            .current
            .scope(&self.serial, serial.provider_generation())
            .is_err()
        {
            self.revoke(KernelRevocationCause::BaseReplaced);
            return Err(MachineRunError::ProtectionDomain(
                crate::protected_region::DomainRefusal::WrongBinding,
            ));
        }
        self.capabilities
            .complete(lease)
            .map_err(MachineRunError::ProtectionCapability)?;
        self.region
            .backend_mut()
            .map_err(MachineRunError::ProtectionDomain)?
            .effect_completed();
        match self
            .region
            .resume(&self.current, 1, &mut self.capabilities)
            .map_err(MachineRunError::ProtectionDomain)?
        {
            DomainReturn::Yielded => {
                if self
                    .region
                    .backend_mut()
                    .map_err(MachineRunError::ProtectionDomain)?
                    .status()
                    != 0
                {
                    self.region
                        .fault(DomainFault::InvalidGate, &mut self.capabilities);
                    return Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate));
                }
                Ok(())
            }
            DomainReturn::Fault(fault) => Err(MachineRunError::ProtectionFault(fault)),
            _ => {
                self.region
                    .fault(DomainFault::InvalidGate, &mut self.capabilities);
                Err(MachineRunError::ProtectionFault(DomainFault::InvalidGate))
            }
        }
    }
}

fn capability_table(generation: u64) -> Result<KernelCapabilityTable, MachineRunError> {
    use crate::cryptographic_entropy::CryptographicEntropyBase;
    let source = crate::arch::DomainEntropy::detect(generation).map_err(|_| {
        MachineRunError::ProtectionDomain(crate::protected_region::DomainRefusal::Unsupported)
    })?;
    let mut entropy = CryptographicEntropyBase::<_, 1>::admit(source)
        .map_err(|_| MachineRunError::KernelConstruction)?;
    let mut key = [0; 8];
    entropy
        .fill(&mut key)
        .map_err(|_| MachineRunError::KernelConstruction)?;
    let table = KernelCapabilityTable::new(u64::from_le_bytes(key))
        .map_err(MachineRunError::ProtectionCapability);
    key.fill(0);
    table
}

impl<I: TextOwner> Drop for ProtectedText<I> {
    fn drop(&mut self) {
        self.revoke(KernelRevocationCause::PlayCancelled);
        use core::fmt::Write;
        let cost = self.region.cost();
        if cost.entries == 0 {
            return;
        }
        let mut sign = crate::sign_format::FixedText::new();
        if writeln!(sign, "CONDUIT_DOMAIN_COST {{\"schema\":\"conduit.conduitos/domain-cost@1\",\"architecture\":\"{}\",\"region_id\":\"{}\",\"plan_id\":\"{}\",\"play_id\":\"{}\",\"domain_id\":{},\"fixture\":{},\"state\":\"{:?}\",\"entries\":{},\"interrupt_entries\":{},\"source_timer_interrupts\":{},\"privilege_transitions\":{},\"gate_transitions\":{},\"copied_bytes\":{},\"setup_copied_bytes\":{},\"base_gate_transitions\":{},\"tlb_flushes\":{},\"setup_ticks\":{},\"teardown_ticks\":{},\"tick_unit\":\"{}\",\"teardown_zeroed_bytes\":{},\"shared_peak_bytes\":{},\"root_metadata_bytes\":{},\"shared_page_bytes\":4096,\"ring_slots\":0,\"address_space_switches\":{},\"scheduler_returns\":{},\"preemptions\":{},\"reserved_bytes\":{},\"dma_isolation\":false,\"driver_isolation\":false}}",
            crate::arch::ARCHITECTURE, self.current.region_id(), self.current.plan_id(),
            self.current.play_id(), self.current.domain().0, self.diagnostic_fixture, self.region.state(),
            cost.entries, cost.interrupt_entries, cost.source_timer_interrupts, cost.privilege_transitions, cost.gate_transitions, cost.copied_bytes, cost.setup_copied_bytes,
            cost.base_gate_transitions, cost.tlb_flushes, cost.setup_ticks, cost.teardown_ticks,
            TextDomain::TICK_UNIT, cost.teardown_zeroed_bytes, cost.shared_peak_bytes, cost.root_metadata_bytes, cost.address_space_switches,
            cost.scheduler_returns, cost.preemptions, cost.reserved_bytes).is_ok() {
            crate::arch::early_write(sign.as_bytes());
        }
    }
}
