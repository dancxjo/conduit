//! One checked entry-description path shared by assigned artifact compilation
//! and inert geometry inspection. No runtime owner or authority is created.
use super::{resident_selection, Inputs};
use crate::{template, workset};
use conduit_core as c;
use conduit_host_make as h;
use conduit_planner as p;
use std::collections::BTreeMap;

/// The original checked entry and native geometry before assignment encoding.
/// Profiles are Rust source for immutable linked schemas, not initialized data.
pub struct InertEntry {
    pub name: String,
    pub lowering: template::InertLowering,
    pub value_profiles_rust: Option<String>,
}
pub fn inspect(workset: &workset::Workset, input: Inputs<'_>) -> Result<Vec<InertEntry>, String> {
    with_entries(workset, &input, |source, _, args| {
        let lowering = template::lower(&args)?;
        let value_profiles_rust = lowering.value_profiles(&args)?;
        Ok(InertEntry {
            name: source.name.clone(),
            lowering,
            value_profiles_rust,
        })
    })
}
pub(super) fn with_entries<T>(
    workset: &workset::Workset,
    input: &Inputs<'_>,
    mut finish: impl FnMut(
        &workset::Entry,
        &[c::CapabilityOffer],
        template::Inputs<'_>,
    ) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    workset
        .installed_values
        .validate_configuration(input.checked_host)?;
    h::verify_configuration_image(input.checked_host, input.image, input.catalog)?;
    if input.entries.len() != workset.entries.len()
        || input.internal_queue.item_capacity == 0
        || input.internal_queue.byte_capacity == 0
        || input.maximum_offer_image_bytes == 0
    {
        return Err(
            "calibration requires one independently bounded selection per checked entry".into(),
        );
    }
    let mut entries = Vec::new();
    let mut resident_owners = std::collections::BTreeSet::new();
    for (source, limits) in workset.entries.iter().zip(input.entries) {
        if source.name != limits.entry {
            return Err("calibration entry order differs from exact checked workset".into());
        }
        let placements = resident_selection::select(
            &source.plot.expanded,
            input.host,
            input.resident,
            &mut resident_owners,
        )?;
        let mut other = Vec::new();
        for placement in placements.by_gear.values() {
            let offer = input
                .host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .ok_or("calibration selected absent capability definition")?;
            if !input
                .resident
                .iter()
                .any(|image| image.offer_description() == offer)
                && !other
                    .iter()
                    .any(|prior: &c::CapabilityOffer| prior.capability_id == offer.capability_id)
            {
                other.push(offer.clone());
            }
        }
        let others: Vec<_> = other.iter().collect();
        let empty = BTreeMap::new();
        let line_candidates = BTreeMap::new();
        let args = template::Inputs {
            plot: &source.plot,
            checked_source: Some(&workset.checked_source),
            host: input.host,
            placements: &placements,
            planning: p::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &line_candidates,
                connection_item_capacity: input.internal_queue.item_capacity,
                connection_byte_capacity: input.internal_queue.byte_capacity,
                authority_grants: input.authority_descriptions,
                protected_resource_grants: input.protected_resource_descriptions,
                line_offers: &[],
            },
            connections: &limits.connections,
            boundaries: &limits.boundaries,
            resident: input.resident,
            other: &others,
            image: input.image_bounds,
            computations: input.computations,
            maxima: input.maxima,
            maximum_fore_ports: input.maximum_fore_ports,
        };
        entries.push(finish(source, &other, args)?);
    }
    Ok(entries)
}
