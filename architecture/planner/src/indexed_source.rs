//! Ordinary planning from an independently authenticated checked Make image.
//! Caller supplies current offers and may select or supply exact placements; this creates no
//! readiness, membership, grant, native provider or execution authority.
use crate::{
    planning_input::{PlanningGears, PlanningInput},
    ConnectionEndpoints, ConnectionQueueLimits, PlacementChoices, PlannerError, PlanningOptions,
};
use alloc::{collections::BTreeMap, vec::Vec};
use conduit_core::{BaseImplementationId, HostAdvertisement, Plan};
use conduit_plot::flat_image::indexed::IndexedSourceImage;

/// Select from current offers with the same ordering, instance admission,
/// language and optional Quantity-wire search as ordinary checked Source.
/// This establishes choices only, never a Plan or execution authority.
pub fn default_placements(
    image: &IndexedSourceImage<'_>,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    let metadata = image.planning_metadata().map_err(|e| {
        PlannerError::InvalidPlotIdentity(alloc::format!("indexed Source metadata: {e:?}"))
    })?;
    crate::functional_compatibility::default_planning_placements(
        &PlanningGears::indexed(image),
        metadata.connections(),
        hosts,
    )
}

pub fn plan_with_connection_limits(
    image: &IndexedSourceImage<'_>,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    let mut metadata = image.planning_metadata().map_err(|e| {
        PlannerError::InvalidPlotIdentity(alloc::format!("indexed Source metadata: {e:?}"))
    })?;
    let connections = metadata.take_connections();
    let identity = metadata.identity();
    let mut plan = crate::validated_planning::plan_input_with_connection_limits(
        PlanningInput {
            source_document_id: &identity.source_document_id,
            checked_plot_id: &identity.checked_plot_id,
            expanded_plot_id: &identity.expanded_plot_id,
            name: metadata.name(),
            completion: metadata.completion(),
            gears: PlanningGears::indexed(image),
            connections: alloc::borrow::Cow::Owned(connections),
        },
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    crate::canonical::attach_fragment_source_spans_by(&mut plan.fragments, |gear| {
        metadata.source_span(gear)
    })?;
    // The indexed Make format explicitly refuses activation, pool and Plot-Back
    // construction. It never omits those facts from an otherwise admitted image.
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            identity,
            crate::plan_completion_policy(metadata.completion()),
            Vec::new(),
            plan.fragments,
        ),
    )
}

mod authoring;
pub use authoring::IndexedAuthoringBoundary;
mod compact;
pub use compact::{
    plan_compact_authoring_consuming_with_connection_limits,
    plan_compact_authoring_with_connection_limits,
    plan_compact_authoring_with_indexed_offers_and_connection_limits,
    plan_compact_consuming_with_connection_limits, plan_compact_with_connection_limits,
    plan_compact_with_indexed_offers_and_connection_limits, CompactIndexedPlan,
};

mod offers;
pub use offers::{IndexedHostOffers, IndexedSoftwareOffers};

/// Select against complete native facts and stable borrowed Make software.
pub fn default_placements_with_indexed_offers(
    image: &IndexedSourceImage<'_>,
    hosts: &[IndexedHostOffers<'_>],
) -> Result<PlacementChoices, PlannerError> {
    let metadata = image.planning_metadata().map_err(|e| {
        PlannerError::InvalidPlotIdentity(alloc::format!("indexed Source metadata: {e:?}"))
    })?;
    crate::functional_compatibility::default_access_placements(
        &PlanningGears::indexed(image),
        metadata.connections(),
        &crate::offer_access::OfferAccess::Indexed(hosts),
    )
}

#[cfg(test)]
pub(crate) mod test_support;
