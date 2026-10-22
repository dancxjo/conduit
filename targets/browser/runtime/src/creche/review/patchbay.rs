//! The only open resident admitted at Birth is the existing exact Patchbay Fore.
//! Ordinary roots keep closed-root review; preparing this sink grants no input.
use conduit_core::ConnectionTrack;

pub(super) fn require_resident_fore(
    plot: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<(), String> {
    let expected = patchbay_application::patchbay_interaction_contract();
    let [owner] = plot.expanded.gears.as_slice() else {
        return Err("reviewed resident Fore requires one exact Patchbay owner".into());
    };
    let [input] = plot.input_bindings.as_slice() else {
        return Err("reviewed resident Fore requires one exact Patchbay input".into());
    };
    if plot.front != expected.checked_front()
        || owner.kind_id != expected.kind_id
        || owner.kind_contract_revision != expected.kind_contract_revision
        || owner.checked_front() != expected.checked_front()
        || !plot.output_bindings.is_empty()
        || !plot.expanded.connections.is_empty()
        || !plot.expanded.activations.is_empty()
        || plot.abnormal_export.is_some()
        || input.front_port_id.as_str() != "interaction"
        || input.gear_id != owner.gear_id
        || input.gear_port_id.as_str() != "interaction"
        || input.track != ConnectionTrack::Payload
    {
        return Err("reviewed resident Fore differs from the canonical Patchbay boundary".into());
    }
    Ok(())
}

/// Check the selected Back and lowered Fore, rather than a copied Kind label.
pub(super) fn require_planned_fore(plan: &conduit_core::Plan) -> Result<(), String> {
    let [fragment] = plan.fragments.as_slice() else {
        return Err("reviewed Patchbay Fore must have one local fragment".into());
    };
    let [placement] = fragment.placements.as_slice() else {
        return Err("reviewed Patchbay Fore must have one installed sink".into());
    };
    crate::installed_browser::patchbay_ingress::prepare(placement)?;
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .map_err(|error| format!("reviewed Patchbay lowering: {error:?}"))?;
    let [input] = lowered.fore_ports.as_slice() else {
        return Err("reviewed Patchbay requires one lowered input Fore".into());
    };
    if input.direction != conduit_core::PortDirection::Input
        || input.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
        || input.temporal != (conduit_core::PortTemporal::Flow { closes: true })
        || input.item_capacity != 1
        || input.byte_capacity != conduit_face::MAX_FACE_INTERACTION_BYTES as u32
        || input.selected_line.is_some()
    {
        return Err("reviewed Patchbay differs from the exact finite interaction Fore".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "planned_fore_tests.rs"]
mod planned_fore_tests;
