//! Lossless private carriers; the complete ordinary Plan is never mirrored.
use conduit_core::{Plan, PlanId, PlotIdentity};
use conduit_face::{FaceContentId, MaskPlot, MaskShow, MaskShowId, PlannedMaskPlot, Show};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct PlannedRef<'a> {
    mask: &'a MaskPlot,
    plan: &'a Plan,
}
#[derive(Serialize)]
pub(super) struct ShowRef<'a> {
    show_id: &'a MaskShowId,
    mask_plot: &'a PlotIdentity,
    face_plot: &'a Option<PlotIdentity>,
    face_plan_id: &'a Option<PlanId>,
    face_id: &'a FaceContentId,
    face_revision: u64,
    planned_mask: PlannedRef<'a>,
    show: &'a Show,
}
impl<'a> From<&'a MaskShow> for ShowRef<'a> {
    fn from(value: &'a MaskShow) -> Self {
        Self {
            show_id: &value.show_id,
            mask_plot: &value.mask_plot,
            face_plot: &value.face_plot,
            face_plan_id: &value.face_plan_id,
            face_id: &value.face_id,
            face_revision: value.face_revision,
            planned_mask: PlannedRef {
                mask: &value.planned_mask.mask,
                plan: &value.planned_mask.plan,
            },
            show: &value.show,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlannedOwned {
    mask: MaskPlot,
    plan: Plan,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShowOwned {
    show_id: MaskShowId,
    mask_plot: PlotIdentity,
    face_plot: Option<PlotIdentity>,
    face_plan_id: Option<PlanId>,
    face_id: FaceContentId,
    face_revision: u64,
    planned_mask: PlannedOwned,
    show: Show,
}
impl From<ShowOwned> for MaskShow {
    fn from(value: ShowOwned) -> Self {
        Self {
            show_id: value.show_id,
            mask_plot: value.mask_plot,
            face_plot: value.face_plot,
            face_plan_id: value.face_plan_id,
            face_id: value.face_id,
            face_revision: value.face_revision,
            planned_mask: PlannedMaskPlot {
                mask: value.planned_mask.mask,
                plan: value.planned_mask.plan,
            },
            show: value.show,
        }
    }
}
