//! Exact resident boundaries: existing Patchbay and one finite Stem operation.
use conduit_body_make::{stem_runtime::*, stem_runtime_catalog::stem_kind};
use conduit_core::{ConnectionTrack, PortDirection, PortTemporal};
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey};
use conduit_plot::ExpandedAuthoringPlot;
use std::collections::BTreeMap;

pub(in crate::creche) fn require_resident_fore(plot: &ExpandedAuthoringPlot) -> Result<(), String> {
    if !plot
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == STEM_KIND)
    {
        return super::patchbay::require_resident_fore(plot);
    }
    let expected = stem_kind();
    let [owner] = plot.expanded.gears.as_slice() else {
        return Err("Stem Fore requires one exact owner".into());
    };
    let [input] = plot.input_bindings.as_slice() else {
        return Err("Stem requires one exact request Fore".into());
    };
    let [output] = plot.output_bindings.as_slice() else {
        return Err("Stem requires one exact outcome Fore".into());
    };
    if plot.front != expected.checked_front()
        || owner.kind_id != expected.kind_id
        || owner.kind_contract_revision != expected.kind_contract_revision
        || !owner.accepts_realization(&crate::installed_browser::stem::offer())
        || !plot.expanded.connections.is_empty()
        || !plot.expanded.activations.is_empty()
        || plot.abnormal_export.is_some()
        || input.gear_id != owner.gear_id
        || output.gear_id != owner.gear_id
        || input.front_port_id.as_str() != "request"
        || input.gear_port_id.as_str() != "request"
        || output.front_port_id.as_str() != "outcome"
        || output.gear_port_id.as_str() != "outcome"
        || input.track != ConnectionTrack::Payload
        || output.track != ConnectionTrack::Payload
    {
        return Err("resident Stem differs from its exact finite Fore".into());
    }
    Ok(())
}

pub(in crate::creche) fn fore_bounds(
    plot: &ExpandedAuthoringPlot,
) -> BTreeMap<ForeBoundaryKey, ConnectionQueueLimits> {
    let mut bounds = crate::installed_browser::patchbay_owner_installation::fore_bounds(plot);
    if plot
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == STEM_KIND)
    {
        for (direction, port, bytes) in [
            (PortDirection::Input, "request", STEM_REQUEST_BYTES),
            (PortDirection::Output, "outcome", STEM_OUTCOME_BYTES),
        ] {
            bounds.insert(
                ForeBoundaryKey {
                    direction,
                    front_port_id: conduit_core::port_id(port),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes as u32,
                },
            );
        }
    }
    bounds
}

pub(super) fn require_planned_fore(plan: &conduit_core::Plan) -> Result<(), String> {
    if !plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .any(|gear| gear.kind_id.as_str() == STEM_KIND)
    {
        return super::patchbay::require_planned_fore(plan);
    }
    let [fragment] = plan.fragments.as_slice() else {
        return Err("Stem requires one exact local fragment".into());
    };
    let [gear] = fragment.placements.as_slice() else {
        return Err("Stem requires one exact local operation".into());
    };
    crate::installed_browser::stem::validate(gear)?;
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .map_err(|error| format!("Stem lowering: {error:?}"))?;
    if lowered.fore_ports.len() != 2 {
        return Err("Stem requires two exact finite Fore ports".into());
    }
    for (direction, kind, bytes) in [
        (PortDirection::Input, STEM_REQUEST_KIND, STEM_REQUEST_BYTES),
        (PortDirection::Output, STEM_OUTCOME_KIND, STEM_OUTCOME_BYTES),
    ] {
        let ports = lowered
            .fore_ports
            .iter()
            .filter(|port| port.direction == direction)
            .collect::<Vec<_>>();
        let [port] = ports.as_slice() else {
            return Err("Stem Fore direction is not exact".into());
        };
        if port.value_kind.as_str() != kind
            || port.temporal != PortTemporal::Value
            || port.item_capacity != 1
            || port.byte_capacity != bytes as u32
            || port.selected_line.is_some()
        {
            return Err("Stem lowered Fore differs from its exact value contract".into());
        }
    }
    Ok(())
}
