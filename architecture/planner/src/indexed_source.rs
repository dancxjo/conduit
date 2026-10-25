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
    let metadata = image.metadata().map_err(|e| {
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
    let metadata = image.metadata().map_err(|e| {
        PlannerError::InvalidPlotIdentity(alloc::format!("indexed Source metadata: {e:?}"))
    })?;
    let identity = metadata.identity();
    let mut plan = crate::validated_planning::plan_input_with_connection_limits(
        PlanningInput {
            source_document_id: &identity.source_document_id,
            checked_plot_id: &identity.checked_plot_id,
            expanded_plot_id: &identity.expanded_plot_id,
            name: metadata.name(),
            completion: metadata.completion(),
            gears: PlanningGears::indexed(image),
            connections: metadata.connections(),
        },
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    crate::canonical::attach_source_spans(metadata.provenance(), &mut plan)?;
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
