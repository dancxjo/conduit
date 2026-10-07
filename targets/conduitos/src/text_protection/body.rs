//! Prepared exact Body ownership; activation uses the real Body Play.
use super::*;
use crate::{domain_serial_scope::SerialScope, protected_region::BodyRegionBinding};
use conduit_body::{BodyPlan, BodyPlayIdentity, ResidentPlot};
use conduit_core::{BootId, ExecutionRegionId, HostId};

#[derive(Clone)]
pub(crate) struct BodyTextAdmission {
    plot: ResidentPlot,
    host: HostId,
    boot: BootId,
    region: ExecutionRegionId,
    serial: SerialScope,
    generation: u64,
}

impl BodyTextAdmission {
    pub fn prepare(
        plan: &BodyPlan,
        plot: &ResidentPlot,
        fixed: &crate::offer::HostOffer<'_>,
    ) -> Result<Self, MachineRunError> {
        let partition = plan
            .plots
            .iter()
            .find(|partition| &partition.plot == plot)
            .ok_or(MachineRunError::KernelConstruction)?;
        let [fragment] = partition.plan.fragments.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
        let [region] = fragment.execution_regions.as_slice() else {
            return Err(MachineRunError::KernelConstruction);
        };
        let serial = SerialScope::prepare_body(
            plan,
            plot,
            &fragment.host_id,
            &fragment.boot_id,
            &region.region_id,
            fixed,
        )
        .map_err(MachineRunError::ProtectionDomain)?;
        Ok(Self {
            plot: plot.clone(),
            host: fragment.host_id.clone(),
            boot: fragment.boot_id.clone(),
            region: region.region_id.clone(),
            serial,
            generation: fixed.generation,
        })
    }

    pub fn activate(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<ProtectedText<BodyRegionBinding>, MachineRunError> {
        let started = TextDomain::ticks();
        let domain = NEXT_DOMAIN
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| MachineRunError::KernelConstruction)?;
        let binding = BodyRegionBinding::admit(
            plan,
            play,
            &self.plot,
            &self.host,
            &self.boot,
            &self.region,
            ProtectionDomainId(domain),
        )
        .map_err(MachineRunError::ProtectionDomain)?;
        let partition = plan
            .plots
            .iter()
            .find(|p| p.plot == self.plot)
            .ok_or(MachineRunError::KernelConstruction)?;
        let region = partition
            .plan
            .fragments
            .iter()
            .flat_map(|f| &f.execution_regions)
            .find(|r| r.region_id == self.region)
            .ok_or(MachineRunError::KernelConstruction)?;
        if region.execution_profile_id.as_str() != crate::ordinary_plan::PROTECTED_REGION_PROFILE
            || !region.isolation_required
            || !region.preemption_required
        {
            return Err(MachineRunError::KernelConstruction);
        }
        let serial = self
            .serial
            .activate_body(plan, &binding)
            .map_err(MachineRunError::ProtectionDomain)?;
        let mut capabilities = capability_table(self.generation)?;
        let serial_handle = capabilities
            .issue(binding.domain, serial.scope)
            .map_err(MachineRunError::ProtectionCapability)?;
        let heap_bytes = body_binding_heap(&binding)
            .checked_mul(2)
            .ok_or(MachineRunError::KernelConstruction)?;
        let root_bytes =
            u32::try_from(core::mem::size_of::<ProtectedText<BodyRegionBinding>>() + heap_bytes)
                .map_err(|_| MachineRunError::KernelConstruction)?;
        if TextDomain::RESERVED_BYTES
            .checked_add(root_bytes)
            .is_none_or(|bytes| bytes > region.requirements.runtime_memory_bytes)
        {
            return Err(MachineRunError::ProtectionDomain(
                crate::protected_region::DomainRefusal::InvalidMemory,
            ));
        }
        let mut backend = TextDomain::install().map_err(MachineRunError::ProtectionDomain)?;
        backend.preparation_cost(started, root_bytes);
        let mut protected = ProtectedText {
            current: binding.clone(),
            region: ProtectedRegion::installed(binding, backend),
            capabilities,
            serial,
            serial_handle,
            diagnostic_fixture: false,
        };
        protected.reset_keymap()?;
        Ok(protected)
    }
}

fn body_binding_heap(binding: &BodyRegionBinding) -> usize {
    binding.active.active_play_id.0.capacity()
        + binding.active.body_id.as_str().len()
        + binding.active.wake_id.as_str().len()
        + binding.active.plan_id.0.capacity()
        + binding.plot.source_document_id.0.capacity()
        + binding.plot.checked_plot_id.0.capacity()
        + binding.partition_plan.0.capacity()
        + binding.host.0.capacity()
        + binding.boot.0.capacity()
        + binding.region.0.capacity()
}
