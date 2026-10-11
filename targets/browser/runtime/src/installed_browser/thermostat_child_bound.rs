use conduit_core::{PlannedScanActivation, PortDirection, PortTemporal};
use conduit_thermostat_plot::{COMMAND_BYTES, STATE_BYTES};
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
    super::thermostat::validate(&fragment.placements[0])?;
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
