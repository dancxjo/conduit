//! Patchbay projection of the public Mask Form boundary and exact ordinary Plan.

use alloc::{string::String, vec::Vec};
use conduit_core::{FormIdentity, PlanId};
use conduit_presentation::{
    AdmittedMaskFormRoutes, MaskPlanningDisposition, MaskReconciliation, MaskShow,
    MaskShowDisposition, MaskWardrobe, PlannedMaskForm, SealedMaskFormRoute, SelectedMaskFormRoute,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskInspectionProjection {
    pub wardrobe: MaskWardrobe,
    pub known_mask_forms: Vec<FormIdentity>,
    pub doffed_mask_forms: Vec<FormIdentity>,
    pub planned_masks: Vec<PlannedMaskForm>,
    pub routes: Vec<MaskInspectionRoute>,
    pub current_show: Option<MaskInspectionShow>,
    pub planning: MaskPlanningDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskInspectionRoute {
    pub route: SealedMaskFormRoute,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskInspectionShow {
    pub route_id: String,
    pub mask_form: FormIdentity,
    pub plan_id: PlanId,
    pub mask_plan_id: PlanId,
    pub show_id: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub show_occurrence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskInspectionError {
    UnknownWornMask,
    MissingPlannedMask,
    StaleSelection,
    MissingShow,
    UnexpectedShow,
    ShowMismatch,
}

pub fn project_mask_inspection(
    wardrobe: &MaskWardrobe,
    known_mask_forms: &[FormIdentity],
    planned_masks: &[PlannedMaskForm],
    routes: &AdmittedMaskFormRoutes,
    reconciliation: &MaskReconciliation,
    show: Option<&MaskShow>,
) -> Result<MaskInspectionProjection, MaskInspectionError> {
    if wardrobe
        .worn
        .iter()
        .any(|worn| !known_mask_forms.contains(worn))
    {
        return Err(MaskInspectionError::UnknownWornMask);
    }
    let selected = selected_route(&reconciliation.show);
    if let Some(selected) = selected {
        if !routes.routes().iter().any(|route| {
            route.route_id == selected.route_id
                && route.mask_form == selected.mask_form
                && route.plan_id == selected.plan_id
        }) {
            return Err(MaskInspectionError::StaleSelection);
        }
    }
    let current_show = project_show(selected, show, planned_masks, routes)?;
    Ok(MaskInspectionProjection {
        wardrobe: wardrobe.clone(),
        known_mask_forms: known_mask_forms.to_vec(),
        doffed_mask_forms: known_mask_forms
            .iter()
            .filter(|mask| !wardrobe.worn.contains(mask))
            .cloned()
            .collect(),
        planned_masks: planned_masks.to_vec(),
        routes: routes
            .routes()
            .iter()
            .cloned()
            .map(|route| MaskInspectionRoute {
                selected: selected.is_some_and(|selected| {
                    selected.route_id == route.route_id
                        && selected.mask_form == route.mask_form
                        && selected.plan_id == route.plan_id
                }),
                route,
            })
            .collect(),
        current_show,
        planning: reconciliation.planning,
    })
}

fn selected_route(disposition: &MaskShowDisposition) -> Option<&SelectedMaskFormRoute> {
    match disposition {
        MaskShowDisposition::Retain(selected)
        | MaskShowDisposition::SelectSealed { selected, .. } => Some(selected),
        MaskShowDisposition::NoCurrentShow { .. } => None,
    }
}

fn project_show(
    selected: Option<&SelectedMaskFormRoute>,
    show: Option<&MaskShow>,
    planned_masks: &[PlannedMaskForm],
    routes: &AdmittedMaskFormRoutes,
) -> Result<Option<MaskInspectionShow>, MaskInspectionError> {
    let (selected, show) = match (selected, show) {
        (Some(selected), Some(show)) => (selected, show),
        (Some(_), None) => return Err(MaskInspectionError::MissingShow),
        (None, Some(_)) => return Err(MaskInspectionError::UnexpectedShow),
        (None, None) => return Ok(None),
    };
    let route = routes
        .routes()
        .iter()
        .find(|route| {
            route.route_id == selected.route_id
                && route.mask_form == selected.mask_form
                && route.plan_id == selected.plan_id
        })
        .ok_or(MaskInspectionError::StaleSelection)?;
    let planned = planned_masks
        .iter()
        .find(|planned| {
            planned.mask.form_identity == selected.mask_form
                && route.placement_ids.len()
                    == planned
                        .plan
                        .fragments
                        .iter()
                        .map(|fragment| fragment.placements.len())
                        .sum::<usize>()
                && route.placement_ids.iter().all(|placement_id| {
                    planned
                        .plan
                        .fragments
                        .iter()
                        .flat_map(|fragment| &fragment.placements)
                        .any(|placement| &placement.placement_id == placement_id)
                })
        })
        .ok_or(MaskInspectionError::MissingPlannedMask)?;
    if show.mask_form != selected.mask_form
        || show.planned_mask != *planned
        || show.show.plan_id != planned.plan.plan_id
        || show.presentation_id != show.show.presentation_id
        || show.presentation_revision != show.show.presentation_revision
    {
        return Err(MaskInspectionError::ShowMismatch);
    }
    Ok(Some(MaskInspectionShow {
        route_id: selected.route_id.clone(),
        mask_form: show.mask_form.clone(),
        plan_id: selected.plan_id.clone(),
        mask_plan_id: planned.plan.plan_id.clone(),
        show_id: show.show_id.as_str().into(),
        presentation_id: show.presentation_id.as_str().into(),
        presentation_revision: show.presentation_revision,
        show_occurrence_id: show.show.manifestation_id.as_str().into(),
    }))
}
