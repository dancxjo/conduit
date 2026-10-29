//! Checked local Mask preparation, without reconstructing a rich Plan or wire.
use super::*;
use conduit_plan_lowering::local_tables::VerifiedLocalLowering;
use conduit_planner::indexed_source::IndexedAuthoringBoundary;
use conduit_plot::flat_image::indexed::IndexedSourceImage;

pub struct LocalMaskInput<'a, 'p> {
    pub verified: &'a VerifiedLocalLowering<'p>,
    /// Independently authenticated checked Make graph, boundary and Mask role.
    pub image: &'a IndexedSourceImage<'a>,
    pub boundary: &'a IndexedAuthoringBoundary,
    pub role: &'a f::MaskPlot,
    /// Actual current Root scope, not identities copied from received metadata.
    pub scope: (&'a c::HostId, &'a c::BootId, c::OfferGeneration),
}
impl PreparedMaskKernel {
    /// Dormant pure-software preparation only. This admits no native provider,
    /// Body/Face route, caller permit, Play, Host Call or electrical effect.
    pub fn prepare_local(input: LocalMaskInput<'_, '_>) -> Result<Self, &'static str> {
        Self::prepare_local_parts(input)?.finish()
    }
    /// Retain only checked owned tables/Backs/Fore, allowing the caller to retire
    /// the complete verified Plan and lowering before canonical slot allocation.
    pub fn prepare_local_parts(
        input: LocalMaskInput<'_, '_>,
    ) -> Result<PreparedMaskKernelParts, &'static str> {
        // All Source/configuration/Cord scratch is retired before kernel storage.
        check(&input)?;
        let (plan, lowered) = input.verified.preparation_scope();
        let fragment = plan
            .fragment(&lowered.identity.fragment_id)
            .ok_or("mask-local-fragment")?;
        Self::prepare_lowered_parts(fragment.lowering_metadata(), lowered, |_| Ok(()))
    }
}
fn check(input: &LocalMaskInput<'_, '_>) -> Result<(), &'static str> {
    let (plan, lowered) = input.verified.preparation_scope();
    let view = plan
        .fragment(&lowered.identity.fragment_id)
        .ok_or("mask-local-fragment")?;
    let fragment = view.lowering_metadata();
    let identity = input.image.identity().map_err(|_| "mask-local-source")?;
    let (host, boot, generation) = input.scope;
    if identity != input.role.plot_identity
        || fragment.source_document_id != identity.source_document_id
        || fragment.checked_plot_id != identity.checked_plot_id
        || fragment.expanded_plot_id != identity.expanded_plot_id
        || fragment.host_id != *host
        || fragment.boot_id != *boot
        || generation.0 == 0
        || fragment.offer_generation != generation
        || plan.fragment_count() != 1
        || input.image.gear_count() != 3
        || fragment.placements.len() != 3
        || fragment.fore_ports.len() != 3
        || lowered.nodes.len() != 3
        || lowered.node_specs.len() != 3
        || !lowered.remote_endpoints.is_empty()
        || !lowered.states.is_empty()
        || !lowered.fusions.is_empty()
        || !lowered.resource_cords.is_empty()
        || !lowered.shared_pools.is_empty()
        || !fragment.execution_regions.is_empty()
        || lowered
            .nodes
            .iter()
            .any(|node| !node.terminal_transductions.is_empty())
        || fragment
            .fore_ports
            .iter()
            .any(|fore| fore.selected_line.is_some())
    {
        return Err("mask-local-scope-or-shape");
    }
    input
        .boundary
        .check_selected_fore(plan)
        .map_err(|_| "mask-local-fore")?;
    for (boundary, direction, kind, temporal) in [
        (
            &input.role.face_input,
            c::PortDirection::Input,
            f::FACE_VALUE_KIND,
            c::PortTemporal::Value,
        ),
        (
            &input.role.show_output,
            c::PortDirection::Output,
            f::SHOW_VALUE_KIND,
            c::PortTemporal::Value,
        ),
        (
            &input.role.interaction_output,
            c::PortDirection::Output,
            f::FACE_INTERACTION_VALUE_KIND,
            c::PortTemporal::Flow { closes: true },
        ),
    ] {
        if fragment
            .fore_ports
            .iter()
            .filter(|fore| {
                fore.front_port_id == boundary.front_port_id
                    && fore.direction == direction
                    && fore.gear_port_id == boundary.gear_port_id
                    && fore.value_kind.as_str() == kind
                    && fore.temporal == temporal
                    && fore.track == c::ConnectionTrack::Payload
                    && fragment.placements.iter().any(|gear| {
                        gear.placement_id == fore.placement_id && gear.gear_id == boundary.gear_id
                    })
            })
            .count()
            != 1
        {
            return Err("mask-local-role");
        }
    }
    {
        let source = input
            .image
            .planning_metadata()
            .map_err(|_| "mask-local-source")?;
        if source.name() != input.role.plot_name
            || matches!(
                source.completion(),
                conduit_plot::PlotCompletionPolicy::SemanticCompletion
            ) != matches!(
                plan.preparation_metadata().completion_policy,
                c::PlanCompletionPolicy::SemanticCompletion
            )
            || fragment.connections.len() != source.connections().len()
        {
            return Err("mask-local-source");
        }
        let gear_id = |placement: &c::PlacementId| {
            fragment
                .placements
                .iter()
                .find(|gear| &gear.placement_id == placement)
                .map(|gear| &gear.gear_id)
        };
        for cord in source.connections() {
            if fragment
                .connections
                .iter()
                .filter(|actual| {
                    gear_id(&actual.source_placement_id) == Some(&cord.source_gear_id)
                        && gear_id(&actual.sink_placement_id) == Some(&cord.sink_gear_id)
                        && actual.source_port_id == cord.source_port_id
                        && actual.sink_port_id == cord.sink_port_id
                        && actual.value_kind == cord.value_kind
                        && actual.track == cord.track
                        && actual.temporal == cord.temporal
                        && actual.resource.is_none()
                        && actual.selected_line.is_none()
                        && actual.admitted_lines.is_empty()
                })
                .count()
                != 1
            {
                return Err("mask-local-cord");
            }
        }
    }
    for index in 0..3 {
        let source = input.image.gear(index).map_err(|_| "mask-local-source")?;
        let gear = fragment
            .placements
            .iter()
            .find(|gear| gear.gear_id == source.gear_id)
            .ok_or("mask-local-gear")?;
        if gear.kind_id != source.kind_id
            || gear.kind_contract_revision != source.kind_contract_revision
            || gear.inputs != source.inputs
            || gear.outputs != source.outputs
            || gear.semantic_contract != source.semantic_contract
            || gear.terminal_transductions != source.terminal_transductions
            || gear.host_id != *host
            || gear.boot_id != *boot
            || gear.offer_generation != generation
            || source.configuration.len() != 1
            || c::face_data_label(&source.configuration).is_err()
            || !view.configuration_matches(&gear.placement_id, &source.configuration)
        {
            return Err("mask-local-contract-or-configuration");
        }
    }
    Ok(())
}
