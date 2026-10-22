//! Prepare the single Patchbay actor at its exact selected owner placement.
use super::super::prepared_patchbay::PreparedPatchbay;
use conduit_body::BodyPlan;
use conduit_core::{ActivePlayId, PlannedGear};

pub(super) fn prepare(
    placement: &PlannedGear,
    plan: &BodyPlan,
    active_play_id: &ActivePlayId,
    source: &str,
    foreground_checked_plot_id: &str,
) -> Result<PreparedPatchbay, String> {
    let residents: Vec<_> = plan.plots.iter().map(|part| part.plot.clone()).collect();
    let subjects = crate::creche::inventory_application_subjects(source, &residents)?;
    let mut active = Vec::with_capacity(plan.plots.len());
    for (part, (expanded, title)) in plan.plots.iter().zip(subjects) {
        active.push(
            patchbay_application::PatchbayActivePlot::project(
                &expanded,
                title,
                part.plot.checked_plot_id.as_str(),
                part.plan.plan_id.clone(),
                patchbay_application::PatchbayActivePlotState::Playing,
                part.plot.checked_plot_id.as_str() == foreground_checked_plot_id,
            )
            .map_err(|error| format!("project resident Patchbay Plot: {error:?}"))?,
        );
    }
    let port = patchbay_application::PatchbayApplicationPort::open_active(
        active,
        plan.plan_id.clone(),
        Some(active_play_id.clone()),
    )
    .map_err(|error| format!("prepare resident Patchbay: {error:?}"))?;
    let resident = plan
        .plots
        .iter()
        .find(|part| {
            part.plan.fragments.iter().any(|fragment| {
                fragment
                    .placements
                    .iter()
                    .any(|gear| gear.placement_id == placement.placement_id)
            })
        })
        .ok_or("Patchbay owner is absent from its Body Plan")?;
    let canonical = placement.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND
        || plan.mask_topologies.iter().any(|topology| {
            topology.face.plot.as_ref() == Some(&resident.plot)
                && topology.face.source_placement_id.is_none()
        });
    let port = if canonical {
        port.with_canvas_delivery(patchbay_application::PatchbayCanvasDelivery::TextInspection)
    } else {
        port
    };
    let mut prepared = PreparedPatchbay::new(port);
    if placement.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND {
        prepared.select_canonical_fore();
    } else if canonical {
        prepared.require_canonical_input();
    }
    Ok(prepared)
}
