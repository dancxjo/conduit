//! Exact std child Back for the authored Thermostat scan.

use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{
    CapabilityOffer, ImplementationId, PlannedGear, PlannedScanActivation, PortDirection,
    PortTemporal,
};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_thermostat_plot::{
    ThermostatBack, COMMAND_BYTES, STATE_BYTES, THERMOSTAT_KIND, THERMOSTAT_REVISION,
};

pub const IMPLEMENTATION: &str = conduit_std_offers::THERMOSTAT_COMBINE_IMPLEMENTATION;

pub struct ThermostatCombineFactory;

pub fn offer() -> CapabilityOffer {
    conduit_std_offers::thermostat_combine_offer()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let expected = offer();
    if placement.kind_id.as_str() != THERMOSTAT_KIND
        || placement.kind_contract_revision.as_str() != THERMOSTAT_REVISION
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Thermostat combine Back differs from the exact std offer".into());
    }
    Ok(())
}

/// The exact one-node Thermostat child has one productive Step and one close Step.
/// Both inputs are admitted and closed before its first scheduler Step, its
/// single output cord has one reserved slot, and there are no internal links
/// or Host Calls that could introduce another wait or retry.
pub(crate) fn maximum_scan_child_steps(planned: &PlannedScanActivation) -> Result<u32, String> {
    let child = planned.selected_plan.as_ref();
    if !conduit_core::verify_plan(child) || child.fragments.len() != 1 {
        return Err(
            "Thermostat scan child Step bound requires one sealed local Plan fragment".into(),
        );
    }
    let fragment = &child.fragments[0];
    if fragment.placements.len() != 1
        || !fragment.connections.is_empty()
        || !fragment.states.is_empty()
        || !fragment.execution_regions.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.shared_pools.is_empty()
        || fragment.expected_terminals
            != [
                conduit_core::ExpectedTerminal::PlacementCompleted(
                    fragment.placements[0].placement_id.clone(),
                ),
                conduit_core::ExpectedTerminal::PlanCompleted,
            ]
        || fragment.terminal_policy
            != conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections
        || fragment.cancellation_policy
            != conduit_core::CancellationPolicy::CancelAllAndRejectLateCompletion
        || fragment.fore_ports.len() != 3
    {
        return Err(
            "Thermostat scan child Step bound requires one pure unlinked combine Gear".into(),
        );
    }
    validate(&fragment.placements[0])?;
    for (expected, direction, maximum_bytes) in [
        (
            &planned.accumulator_input,
            PortDirection::Input,
            STATE_BYTES,
        ),
        (&planned.item_input, PortDirection::Input, COMMAND_BYTES),
        (&planned.output, PortDirection::Output, STATE_BYTES),
    ] {
        if !fragment.fore_ports.iter().any(|front| {
            front.front_port_id == expected.front_port_id
                && front.direction == direction
                && front.placement_id == fragment.placements[0].placement_id
                && front.gear_port_id == expected.front_port_id
                && front.value_kind == expected.value_kind
                && front.abnormal_kind.is_none()
                && front.track == conduit_core::ConnectionTrack::Payload
                && front.temporal == PortTemporal::Value
                && front.item_capacity == 1
                && front.byte_capacity >= maximum_bytes as u32
                && front.selected_line.is_none()
        }) {
            return Err("Thermostat scan child Step bound requires exact local Value Fore".into());
        }
    }
    u32::from(planned.limits.maximum_items)
        .checked_mul(2)
        .ok_or_else(|| "Thermostat scan child Step bound overflow".into())
}

impl KernelOperationFactory for ThermostatCombineFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| ImplementationId::from(IMPLEMENTATION))
    }

    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        validate(placement)?;
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: (2 * STATE_BYTES + COMMAND_BYTES) as u32,
            maximum_value_bytes: STATE_BYTES as u32,
            host_requests: 0,
            sign_items: 3,
        })
    }

    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        validate(placement)?;
        Ok(Box::new(ThermostatBack::new()))
    }
}
