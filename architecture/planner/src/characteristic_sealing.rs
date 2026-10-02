use crate::PlannerError;
use conduit_core::{seal_plan_with_completion, Plan, PlotIdentity, RealizationAdvertisement};

pub(crate) fn seal_characteristics(
    mut plan: Plan,
    advertisements: &[RealizationAdvertisement],
) -> Result<Plan, PlannerError> {
    for fragment in &mut plan.fragments {
        for gear in &mut fragment.placements {
            if let Some(advertisement) = advertisements.iter().find(|item| {
                item.host_id == gear.host_id
                    && item.boot_id == gear.boot_id
                    && item.offer_generation == gear.offer_generation
                    && item.capability_id == gear.capability_id
            }) {
                gear.realization_characteristics = advertisement.characteristics.clone();
                gear.realization_characteristics.sort();
            }
        }
    }
    Ok(seal_plan_with_completion(
        PlotIdentity {
            source_document_id: plan.source_document_id,
            checked_plot_id: plan.checked_plot_id,
            expanded_plot_id: plan.expanded_plot_id,
        },
        plan.completion_policy,
        plan.fragments,
    ))
}
