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
