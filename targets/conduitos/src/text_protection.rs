//! Root preparation and lifecycle for the ordinary text region's implementation.
use crate::{
    arch::TextDomain,
    composition::MachineRunError,
    protected_region::{DomainReturn, ProtectedRegion, RegionBinding},
    protection_domain::{KernelCapabilityTable, KernelRevocationCause, ProtectionDomainId},
    text_upper::{MAXIMUM_BYTES, UppercaseText},
};
use conduit_core::{ActivePlayIdentity, Plan};
use core::sync::atomic::{AtomicU32, Ordering};

static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(1);

pub(crate) struct ProtectedText {
    region: ProtectedRegion<TextDomain>,
    capabilities: KernelCapabilityTable,
    current: RegionBinding,
}

impl ProtectedText {
    pub fn prepare(plan: &Plan, active: &ActivePlayIdentity) -> Result<Self, MachineRunError> {
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
        let [region] = fragment.execution_regions.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
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
        Ok(Self {
            current: binding.clone(),
            region: ProtectedRegion::installed(binding, backend),
            capabilities: KernelCapabilityTable::new(1)
                .map_err(|_| MachineRunError::KernelConstruction)?,
        })
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
            _ => return Err(MachineRunError::KernelFailure),
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
        Ok(output)
    }

    pub fn revoke(&mut self, cause: KernelRevocationCause) {
        self.region.revoke(cause, &mut self.capabilities);
    }
}

impl Drop for ProtectedText {
    fn drop(&mut self) {
        self.revoke(KernelRevocationCause::PlayCancelled);
        use core::fmt::Write;
        let cost = self.region.cost();
        if cost.entries == 0 {
            return;
        }
        let mut sign = crate::sign_format::FixedText::new();
        if writeln!(sign, "CONDUIT_DOMAIN_COST {{\"schema\":\"conduit.conduitos/domain-cost@1\",\"architecture\":\"x86_64\",\"region_id\":\"{}\",\"plan_id\":\"{}\",\"play_id\":\"{}\",\"domain_id\":{},\"state\":\"{:?}\",\"entries\":{},\"gate_transitions\":{},\"copied_bytes\":{},\"address_space_switches\":{},\"scheduler_returns\":{},\"preemptions\":{},\"reserved_bytes\":{},\"dma_isolation\":false,\"driver_isolation\":false}}",
            self.current.region.as_str(), self.current.active.plan_id.as_str(),
            self.current.active.active_play_id.as_str(), self.current.domain.0, self.region.state(),
            cost.entries, cost.gate_transitions, cost.copied_bytes, cost.address_space_switches,
            cost.scheduler_returns, cost.preemptions, cost.reserved_bytes).is_ok() {
            crate::arch::early_write(sign.as_bytes());
        }
    }
}
