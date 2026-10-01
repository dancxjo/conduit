//! Body-plan admission and current availability for ordinary spoken Mask routes.

use super::SpokenMaskExecution;

pub struct SpokenMaskRouteSet {
    pub wake: conduit_body::Wake,
    pub body_plan: conduit_body::BodyPlan,
    pub routes: conduit_presentation::AdmittedMaskFormRoutes,
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
        Body, BodyFaceSelector, BodyFormPlan, BodyMaskChainPlan, BodyMaskTopology, BodyWorkset,
        ResidentForm,
    };
    let planned = executions
        .iter()
        .map(|execution| {
            conduit_presentation::PlannedMaskForm::admit(&execution.mask, &execution.plan)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("admit planned spoken Mask: {error:?}"))?;
    let residents = planned
        .iter()
        .map(|item| {
            ResidentForm::new(
                item.mask.form_identity.source_document_id.clone(),
                item.mask.form_identity.checked_form_id.clone(),
            )
        })
        .collect::<Vec<_>>();
    let wake = if let Some(wake) = existing_wake {
        wake.clone()
    } else {
        let body = Body::born_with_forms(
            BodyWorkset::from_forms(residents.clone())
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
    let forms = residents
        .into_iter()
        .zip(&planned)
        .map(|(form, item)| BodyFormPlan {
            form,
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
                    form: Some(ResidentForm::new(
                        item.mask.form_identity.source_document_id.clone(),
                        item.mask.form_identity.checked_form_id.clone(),
                    )),
                    source_placement_id: first.clone(),
                },
                chains: vec![BodyMaskChainPlan {
                    plan: item.plan.clone(),
                    stage_placement_ids: vec![first],
                }],
            }
        })
        .collect();
    let body_plan = conduit_body::BodyPlan::seal_with_masks(&wake, forms, topologies)
        .map_err(|error| format!("seal spoken Mask Body Plan: {error:?}"))?;
    if availability.len() != planned.len() {
        return Err("spoken Mask route availability count does not match executions".into());
    }
    let routes = planned
        .iter()
        .enumerate()
        .zip(availability)
        .map(
            |((index, item), currently_available)| conduit_presentation::SealedMaskFormRoute {
                route_id: format!("route/{evidence_id}/{index}"),
                mask_form: item.mask.form_identity.clone(),
                plan_id: body_plan.plan_id.clone(),
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
    let routes = conduit_presentation::AdmittedMaskFormRoutes::new(&body_plan, &planned, routes)
        .map_err(|error| format!("admit spoken Mask routes: {error:?}"))?;
    Ok(SpokenMaskRouteSet {
        wake,
        body_plan,
        routes,
    })
}
