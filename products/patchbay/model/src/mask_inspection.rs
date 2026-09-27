//! Bounded Patchbay inspection of portable Masks and their exact realizations.

use conduit_core::{PlacementId, PlanId};
use conduit_presentation::{
    MaskBoundaryRole, MaskPlanningDisposition, MaskReconciliation, MaskShow, MaskShowDisposition,
    MaskSpecification, MaskSpecificationId, MaskWardrobe, PlannedMask, SealedMaskRoute,
    SelectedMaskRoute,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskInspectionProjection {
    pub wardrobe: MaskWardrobe,
    /// Portable meaning, kept distinct from exact implementation choices.
    pub specifications: Vec<MaskSpecification>,
    /// Exact Back, Host, Boot, resource, Cord, and Line facts selected by Plan.
    pub planned_masks: Vec<PlannedMask>,
    /// Every eligible path already sealed into the same immutable Plan.
    pub routes: Vec<MaskInspectionRoute>,
    /// Current public Show correlation, when a Show exists.
    pub current_show: Option<MaskInspectionShow>,
    /// A current availability fact. This is not planning authority.
    pub planning: MaskPlanningDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskInspectionRoute {
    pub route: SealedMaskRoute,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskInspectionShow {
    pub route_id: String,
    pub specification_id: MaskSpecificationId,
    pub plan_id: PlanId,
    pub show_id: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub manifestation_id: String,
    pub terminal_placement_id: PlacementId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskInspectionError {
    DuplicateSpecification,
    DuplicatePlannedMask,
    MissingSpecification,
    MissingPlannedMask,
    SpecificationMismatch,
    PlanMismatch,
    InvalidRoutePath,
    StaleSelection,
    MissingShow,
    UnexpectedShow,
    ShowMismatch,
}

impl core::fmt::Display for MaskInspectionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid Mask inspection: {self:?}")
    }
}

impl std::error::Error for MaskInspectionError {}

pub fn project_mask_inspection(
    wardrobe: &MaskWardrobe,
    specifications: &[MaskSpecification],
    planned_masks: &[PlannedMask],
    routes: &[SealedMaskRoute],
    reconciliation: &MaskReconciliation,
    show: Option<&MaskShow>,
) -> Result<MaskInspectionProjection, MaskInspectionError> {
    validate_catalogs(specifications, planned_masks)?;
    for route in routes {
        validate_route(route, specifications, planned_masks)?;
    }

    let selected = selected_route(&reconciliation.show);
    if let Some(selected) = selected {
        if !routes.iter().any(|route| {
            route.route_id == selected.route_id
                && route.specification_id == selected.specification_id
                && route.plan_id == selected.plan_id
        }) {
            return Err(MaskInspectionError::StaleSelection);
        }
    }
    let current_show = project_show(selected, show, planned_masks)?;

    Ok(MaskInspectionProjection {
        wardrobe: wardrobe.clone(),
        specifications: specifications.to_vec(),
        planned_masks: planned_masks.to_vec(),
        routes: routes
            .iter()
            .cloned()
            .map(|route| MaskInspectionRoute {
                selected: selected.is_some_and(|selected| {
                    selected.route_id == route.route_id
                        && selected.specification_id == route.specification_id
                        && selected.plan_id == route.plan_id
                }),
                route,
            })
            .collect(),
        current_show,
        planning: reconciliation.planning,
    })
}

fn validate_catalogs(
    specifications: &[MaskSpecification],
    planned_masks: &[PlannedMask],
) -> Result<(), MaskInspectionError> {
    for (index, specification) in specifications.iter().enumerate() {
        if specifications[index + 1..]
            .iter()
            .any(|other| other.specification_id == specification.specification_id)
        {
            return Err(MaskInspectionError::DuplicateSpecification);
        }
    }
    for (index, planned) in planned_masks.iter().enumerate() {
        if planned_masks[index + 1..].iter().any(|other| {
            other.specification_id == planned.specification_id && other.plan_id == planned.plan_id
        }) {
            return Err(MaskInspectionError::DuplicatePlannedMask);
        }
        let specification = specifications
            .iter()
            .find(|specification| specification.specification_id == planned.specification_id)
            .ok_or(MaskInspectionError::MissingSpecification)?;
        if specification.revision != planned.specification_revision {
            return Err(MaskInspectionError::SpecificationMismatch);
        }
    }
    Ok(())
}

fn validate_route(
    route: &SealedMaskRoute,
    specifications: &[MaskSpecification],
    planned_masks: &[PlannedMask],
) -> Result<(), MaskInspectionError> {
    let specification = specifications
        .iter()
        .find(|specification| specification.specification_id == route.specification_id)
        .ok_or(MaskInspectionError::MissingSpecification)?;
    let planned = planned_masks
        .iter()
        .find(|planned| {
            planned.specification_id == route.specification_id && planned.plan_id == route.plan_id
        })
        .ok_or(MaskInspectionError::MissingPlannedMask)?;
    if planned.specification_revision != specification.revision {
        return Err(MaskInspectionError::SpecificationMismatch);
    }
    if route.stage_ids.iter().any(|stage_id| {
        !planned
            .stages
            .iter()
            .any(|stage| stage.stage_id == *stage_id)
    }) {
        return Err(MaskInspectionError::InvalidRoutePath);
    }
    if route.stage_ids.windows(2).any(|pair| {
        !planned
            .cords
            .iter()
            .any(|cord| cord.source_stage_id == pair[0] && cord.sink_stage_id == pair[1])
    }) {
        return Err(MaskInspectionError::InvalidRoutePath);
    }
    let starts_at_presentation = specification.boundaries.iter().any(|boundary| {
        boundary.role == MaskBoundaryRole::PresentationInput
            && route.stage_ids.first() == Some(&boundary.stage_id)
    });
    let ends_at_show = specification.boundaries.iter().any(|boundary| {
        boundary.role == MaskBoundaryRole::ShowOutput
            && route.stage_ids.last() == Some(&boundary.stage_id)
    });
    if !starts_at_presentation || !ends_at_show {
        return Err(MaskInspectionError::InvalidRoutePath);
    }
    Ok(())
}

fn selected_route(disposition: &MaskShowDisposition) -> Option<&SelectedMaskRoute> {
    match disposition {
        MaskShowDisposition::Retain(selected)
        | MaskShowDisposition::SelectSealed { selected, .. } => Some(selected),
        MaskShowDisposition::NoCurrentShow { .. } => None,
    }
}

fn project_show(
    selected: Option<&SelectedMaskRoute>,
    show: Option<&MaskShow>,
    planned_masks: &[PlannedMask],
) -> Result<Option<MaskInspectionShow>, MaskInspectionError> {
    let (selected, show) = match (selected, show) {
        (Some(selected), Some(show)) => (selected, show),
        (Some(_), None) => return Err(MaskInspectionError::MissingShow),
        (None, Some(_)) => return Err(MaskInspectionError::UnexpectedShow),
        (None, None) => return Ok(None),
    };
    let planned = planned_masks
        .iter()
        .find(|planned| {
            planned.specification_id == selected.specification_id
                && planned.plan_id == selected.plan_id
        })
        .ok_or(MaskInspectionError::MissingPlannedMask)?;
    if show.specification_id != selected.specification_id
        || show.planned_mask != *planned
        || show.manifestation.plan_id != selected.plan_id
        || show.presentation_id != show.manifestation.presentation_id
        || show.presentation_revision != show.manifestation.presentation_revision
    {
        return Err(MaskInspectionError::ShowMismatch);
    }
    Ok(Some(MaskInspectionShow {
        route_id: selected.route_id.clone(),
        specification_id: show.specification_id.clone(),
        plan_id: selected.plan_id.clone(),
        show_id: show.show_id.as_str().into(),
        presentation_id: show.presentation_id.as_str().into(),
        presentation_revision: show.presentation_revision,
        manifestation_id: show.manifestation.manifestation_id.as_str().into(),
        terminal_placement_id: show.manifestation.placement_id.clone(),
    }))
}
