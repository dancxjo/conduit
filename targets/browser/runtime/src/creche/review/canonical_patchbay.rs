//! The one supported resident input Fore, reviewed through its installed Back.
use conduit_core::{
    BaseImplementationId, ConnectionTrack, HostAdvertisement, PortDirection, PortTemporal,
};
use conduit_planner::{PlacementChoices, PlanningOptions};
use conduit_plot::{
    CanonicalBackCatalog, CheckedSyntaxDocument, ExpandedCanonicalPlot, ProfileCatalog,
};
use std::collections::BTreeMap;

pub(super) fn review(
    document: &CheckedSyntaxDocument,
    name: &str,
    profile: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
) -> Result<(ExpandedCanonicalPlot, PlacementChoices), String> {
    let authored = conduit_plot::expand_canonical_plot_for_authoring_with_backs(
        document, name, profile, backs,
    )
    .map_err(|error| format!("expand reviewed resident Fore: {error:?}"))?;
    let ([gear], [binding]) = (
        authored.expanded.gears.as_slice(),
        authored.input_bindings.as_slice(),
    ) else {
        return Err("initial workload requires one canonical Patchbay sink and input Fore".into());
    };
    if gear.kind_id.as_str() != patchbay_application::PATCHBAY_INTERACTION_KIND
        || !authored.output_bindings.is_empty()
        || binding.front_port_id != conduit_core::port_id("interaction")
        || binding.gear_id != gear.gear_id
        || binding.gear_port_id != conduit_core::port_id("interaction")
        || binding.track != ConnectionTrack::Payload
    {
        return Err("initial workload has an unsupported resident Fore".into());
    }
    let placements = conduit_planner::default_expanded_placements(&authored.expanded, hosts)
        .map_err(|error| error.to_string())?;
    let plan = conduit_planner::plan_expanded_authoring_with_activations(
        document,
        &authored,
        profile,
        backs,
        hosts,
        &placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_face::MAX_FACE_INTERACTION_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &crate::installed_browser::patchbay_owner_installation::fore_bounds(&authored),
    )
    .map_err(|error| error.to_string())?;
    let [fragment] = plan.fragments.as_slice() else {
        return Err("reviewed Patchbay Fore must have one local fragment".into());
    };
    let [placement] = fragment.placements.as_slice() else {
        return Err("reviewed Patchbay Fore must have one installed sink".into());
    };
    // Reuse exact capability/revision/Host Call/storage admission, not a copied
    // Kind name or a broader allowance for arbitrary open-front Plots.
    crate::installed_browser::patchbay_ingress::prepare(placement)?;
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
        .map_err(|error| format!("reviewed Patchbay lowering: {error:?}"))?;
    let [input] = lowered.fore_ports.as_slice() else {
        return Err("reviewed Patchbay requires one lowered input Fore".into());
    };
    if input.direction != PortDirection::Input
        || input.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
        || input.temporal != (PortTemporal::Flow { closes: true })
        || input.item_capacity != 1
        || input.byte_capacity != conduit_face::MAX_FACE_INTERACTION_BYTES as u32
        || input.selected_line.is_some()
    {
        return Err("reviewed Patchbay differs from the exact finite interaction Fore".into());
    }
    Ok((authored.expanded, placements))
}

#[cfg(test)]
#[path = "canonical_patchbay_tests.rs"]
mod tests;
