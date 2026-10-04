//! Body-plan admission and current availability for ordinary spoken Mask routes.

use super::SpokenMaskExecution;

pub struct SpokenMaskRouteSet {
    pub wake: conduit_body::Wake,
    pub body_plan: conduit_body::BodyPlan,
    pub routes: conduit_presentation::AdmittedMaskPlotRoutes,
}

pub fn admit_spoken_mask_routes(
    executions: &[SpokenMaskExecution],
    evidence_id: &str,
) -> Result<SpokenMaskRouteSet, String> {
    admit_spoken_mask_routes_with_availability(
        executions,
        evidence_id,
        &vec![true; executions.len()],
    )
}

pub fn admit_spoken_mask_routes_with_availability(
    executions: &[SpokenMaskExecution],
    evidence_id: &str,
    availability: &[bool],
) -> Result<SpokenMaskRouteSet, String> {
    admit_spoken_mask_routes_for_wake(executions, evidence_id, availability, None)
}

pub fn admit_replacement_spoken_mask_routes(
    executions: &[SpokenMaskExecution],
    evidence_id: &str,
    availability: &[bool],
    wake: &conduit_body::Wake,
) -> Result<SpokenMaskRouteSet, String> {
    admit_spoken_mask_routes_for_wake(executions, evidence_id, availability, Some(wake))
}

fn admit_spoken_mask_routes_for_wake(
    executions: &[SpokenMaskExecution],
    evidence_id: &str,
    availability: &[bool],
    existing_wake: Option<&conduit_body::Wake>,
) -> Result<SpokenMaskRouteSet, String> {
    use conduit_body::{
        Body, BodyFaceSelector, BodyMaskChainPlan, BodyMaskTopology, BodyPlotPlan, BodyWorkset,
        ResidentPlot,
    };
    let planned = executions
        .iter()
        .map(|execution| {
            conduit_presentation::PlannedMaskPlot::admit(&execution.mask, &execution.plan)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("admit planned spoken Mask: {error:?}"))?;
    let residents = planned
        .iter()
        .map(|item| {
            ResidentPlot::new(
                item.mask.plot_identity.source_document_id.clone(),
                item.mask.plot_identity.checked_plot_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    let wake = if let Some(wake) = existing_wake {
        wake.clone()
    } else {
        let body = Body::born_with_plots(
            BodyWorkset::from_plots(residents.clone())
                .map_err(|error| format!("spoken Mask workset: {error:?}"))?,
            1,
            conduit_core::SignId::from(format!("sign/{evidence_id}/born")),
        )
        .map_err(|error| format!("spoken Mask Body: {error:?}"))?;
        body.wake(
            1,
            conduit_core::SignId::from(format!("sign/{evidence_id}/wake")),
        )
        .map_err(|error| format!("spoken Mask Wake: {error:?}"))?
        .1
    };
    let plots = residents
        .into_iter()
        .zip(&planned)
        .map(|(plot, item)| BodyPlotPlan {
            plot,
            plan: item.plan.clone(),
        })
        .collect();
    let topologies = planned
        .iter()
        .map(|item| {
            let first = item
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .next()
                .expect("planned Mask has placement")
                .placement_id
                .clone();
            BodyMaskTopology {
                face: BodyFaceSelector {
                    plot: Some(ResidentPlot::new(
                        item.mask.plot_identity.source_document_id.clone(),
                        item.mask.plot_identity.checked_plot_id.clone(),
                    )),
                    source_placement_id: None,
                },
                chains: vec![BodyMaskChainPlan {
                    plan: item.plan.clone(),
                    stage_placement_ids: vec![first],
                }],
            }
        })
        .collect();
    let body_plan = conduit_body::BodyPlan::seal_with_masks(&wake, plots, topologies)
        .map_err(|error| format!("seal spoken Mask Body Plan: {error:?}"))?;
    if availability.len() != planned.len() {
        return Err("spoken Mask route availability count does not match executions".into());
    }
    let routes = planned
        .iter()
        .enumerate()
        .zip(availability)
        .map(
            |((index, item), currently_available)| conduit_presentation::SealedMaskPlotRoute {
                route_id: format!("route/{evidence_id}/{index}"),
                mask_plot: item.mask.plot_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
                child_mask_plan_id: None,
                owner_route_seal_id: None,
                placement_ids: item
                    .plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| &fragment.placements)
                    .map(|placement| placement.placement_id.clone())
                    .collect(),
                currently_available: *currently_available,
            },
        )
        .collect();
    let routes = conduit_presentation::AdmittedMaskPlotRoutes::new(&body_plan, &planned, routes)
        .map_err(|error| format!("admit spoken Mask routes: {error:?}"))?;
    Ok(SpokenMaskRouteSet {
        wake,
        body_plan,
        routes,
    })
}
