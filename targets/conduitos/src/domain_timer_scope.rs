//! Root admission of a finite timer slot and its separately owned clock basis.
use crate::{
    domain_scope_identity::{identity, parse_identity},
    machine::BaseKind,
    offer::{BaseProviderBinding, HostOffer},
    protected_region::{DomainRefusal, RegionBinding},
    protection_domain::KernelCapabilityScope,
};
use conduit_core::Plan;

pub const TIMER_WAIT_OPERATION: u32 = 10;

#[derive(Clone, Copy)]
pub struct TimerScope {
    pub scope: KernelCapabilityScope,
    pub timer: BaseProviderBinding,
    pub clock: BaseProviderBinding,
}

impl TimerScope {
    pub fn admit(
        plan: &Plan,
        binding: &RegionBinding,
        fixed: &HostOffer<'_>,
    ) -> Result<Self, DomainRefusal> {
        RegionBinding::admit(plan, &binding.active, &binding.region, binding.domain)?;
        fixed.validate().map_err(|_| DomainRefusal::WrongBinding)?;
        if parse_identity(binding.active.host_id.as_str())? != fixed.host_id
            || parse_identity(binding.active.boot_id.as_str())? != fixed.boot_id
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id == binding.active.host_id
                    && fragment.boot_id == binding.active.boot_id
            })
            .ok_or(DomainRefusal::WrongBinding)?;
        if fragment.offer_generation.0 != fixed.generation {
            return Err(DomainRefusal::WrongBinding);
        }
        let region = fragment
            .execution_regions
            .iter()
            .find(|region| region.region_id == binding.region)
            .ok_or(DomainRefusal::WrongBinding)?;
        let mut placements = fragment.placements.iter().filter(|placement| {
            placement.kind_id.as_str() == conduit_time::TIME_EVERY_KIND
                && region.admitted_placements.contains(&placement.placement_id)
        });
        let placement = placements.next().ok_or(DomainRefusal::WrongBinding)?;
        if placements.next().is_some() || !placement.authority.is_empty() {
            return Err(DomainRefusal::WrongBinding);
        }
        let capability = fixed
            .capabilities
            .iter()
            .find(|capability| {
                capability.kind == placement.kind_id.as_str()
                    && capability.implementation == placement.implementation_id.as_str()
                    && capability.implementation == crate::offer::TIME_EVERY_IMPLEMENTATION
                    && capability.required_base == BaseKind::Timer
                    && capability.secondary_base == Some(BaseKind::Clock)
            })
            .ok_or(DomainRefusal::WrongBinding)?;
        if capability.maximum_in_flight != 1
            || capability.maximum_input_bytes != 8
            || capability.maximum_output_bytes != 8
            || capability.input.is_some()
            || capability.output.is_none_or(|port| {
                port.name != "tick"
                    || port.value_kind != conduit_time::TICK_VALUE_KIND
                    || port.direction != crate::offer::PortDirection::Output
                    || port.closes
            })
            || capability.contract_revision != placement.kind_contract_revision.as_str()
            || placement.artifact_id.as_str()
                != alloc::format!("conduitos-build/{}", capability.artifact_build)
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let timer = fixed
            .capability_provider(capability)
            .map_err(|_| DomainRefusal::WrongBinding)?;
        fixed
            .require_base_provider(timer)
            .map_err(|_| DomainRefusal::WrongBinding)?;
        let paired = crate::ordinary_base::timer_clock_provider(fixed)
            .map_err(|_| DomainRefusal::WrongBinding)?;
        let selected = placement.base.as_ref().ok_or(DomainRefusal::WrongBinding)?;
        if selected.base_id.as_str() != crate::identity::hex(&timer.base_id)
            || selected.provider_instance_id.as_str()
                != crate::identity::hex(&paired.provider_instance_id)
            || selected.provider_generation != timer.provider_generation
            || selected.implementation_id.as_str()
                != crate::ordinary_base::TIMER_PROVIDER_IMPLEMENTATION
            || selected.mechanism_family.as_str() != BaseKind::Timer.as_str()
            || selected.enforcement_class != conduit_core::BaseEnforcementClass::Cooperative
        {
            return Err(DomainRefusal::WrongBinding);
        }
        // The selected provider instance seals Every's required secondary clock.
        // Retain its physical identity independently for current epoch checks.
        let clock = fixed
            .bases
            .iter()
            .find(|base| base.kind == BaseKind::Clock)
            .ok_or(DomainRefusal::WrongBinding)?;
        let clock = BaseProviderBinding {
            base_id: clock.id,
            provider_instance_id: clock.provider_instance_id,
            provider_generation: clock.provider_generation,
            kind: BaseKind::Clock,
        };
        fixed
            .require_base_provider(clock)
            .map_err(|_| DomainRefusal::WrongBinding)?;
        let [call] = placement.host_calls.as_slice() else {
            return Err(DomainRefusal::WrongBinding);
        };
        if call.contract_id.as_str() != conduit_core::WAIT_HOST_CALL_CONTRACT
            || Some(call.contract_id.as_str()) != capability.host_call
            || call.maximum_input_bytes != 8
            || call.maximum_output_bytes != 0
            || call.maximum_in_flight != 1
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let mut resources = placement
            .resources
            .iter()
            .filter(|resource| resource.class_id.as_str() == conduit_core::TIMER_RESOURCE_CLASS);
        let resource = resources.next().ok_or(DomainRefusal::WrongBinding)?;
        if resources.next().is_some()
            || resource.units != 1
            || resource.protected.is_some()
            || !fixed.resources.iter().enumerate().any(|(index, offered)| {
                offered.base == BaseKind::Timer
                    && offered.class == resource.class_id.as_str()
                    && resource.pool_id.as_str()
                        == alloc::format!("conduitos-pool-{index}-{}", BaseKind::Timer.as_str())
                    && resource.units <= offered.capacity
            })
        {
            return Err(DomainRefusal::WrongBinding);
        }
        let scope = KernelCapabilityScope {
            host: fixed.host_id,
            boot: fixed.boot_id,
            plan: parse_identity(binding.active.plan_id.as_str())?,
            play: parse_identity(binding.active.active_play_id.as_str())?,
            implementation: identity(
                b"implementation",
                &[
                    placement.implementation_id.as_str().as_bytes(),
                    placement.artifact_id.as_str().as_bytes(),
                ],
            ),
            base: timer.base_id,
            base_generation: u32::try_from(timer.provider_generation)
                .map_err(|_| DomainRefusal::WrongBinding)?,
            resource: identity(b"resource", &[resource.pool_id.as_str().as_bytes()]),
            resource_generation: u32::try_from(fixed.generation)
                .map_err(|_| DomainRefusal::WrongBinding)?,
            operation: TIMER_WAIT_OPERATION,
            subject: identity(
                b"subject",
                &[
                    placement.placement_id.as_str().as_bytes(),
                    placement.gear_id.as_str().as_bytes(),
                    placement.kind_id.as_str().as_bytes(),
                ],
            ),
            authority: identity(
                b"local-selected-timer-clock",
                &[
                    placement.capability_id.as_str().as_bytes(),
                    call.contract_id.as_str().as_bytes(),
                    &clock.base_id,
                    &clock.provider_instance_id,
                    &clock.provider_generation.to_le_bytes(),
                ],
            ),
            maximum_parameter_bytes: 8,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 2,
        };
        Ok(Self {
            scope,
            timer,
            clock,
        })
    }

    /// Current Root provider observations, never epochs copied from a domain frame.
    pub fn current(
        &self,
        binding: &RegionBinding,
        timer_generation: Option<u64>,
        clock_generation: Option<u64>,
    ) -> Result<KernelCapabilityScope, DomainRefusal> {
        if timer_generation != Some(self.timer.provider_generation)
            || clock_generation != Some(self.clock.provider_generation)
            || parse_identity(binding.active.host_id.as_str())? != self.scope.host
            || parse_identity(binding.active.boot_id.as_str())? != self.scope.boot
            || parse_identity(binding.active.plan_id.as_str())? != self.scope.plan
            || parse_identity(binding.active.active_play_id.as_str())? != self.scope.play
        {
            return Err(DomainRefusal::WrongBinding);
        }
        Ok(self.scope)
    }
}

#[cfg(test)]
mod tests;
