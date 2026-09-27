//! One Body-scoped control seam for Mask wardrobe actions and reconciliation.

use conduit_body::{BodyId, WakeId};
use conduit_core::PlanId;
use conduit_presentation::{
    BodyMaskWardrobe, MaskReconciliation, MaskShowDisposition, MaskSpecificationId, MaskWardrobe,
    MaskWardrobeError, SealedMaskRoute, SelectedMaskRoute,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskWardrobeAction {
    Wear(MaskSpecificationId),
    Doff(MaskSpecificationId),
    Prefer(Vec<MaskSpecificationId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskWardrobeControlEvidence {
    pub body_id: BodyId,
    pub wake_id: Option<WakeId>,
    pub action: MaskWardrobeAction,
    pub basis_revision: u64,
    pub resulting_wardrobe: MaskWardrobe,
    pub active_plan_id: PlanId,
    pub reconciliation: MaskReconciliation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskWardrobeControl {
    pub scoped_wardrobe: BodyMaskWardrobe,
    pub active_plan_id: PlanId,
    pub selected: Option<SelectedMaskRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskWardrobeControlError {
    WrongBody,
    StalePlan,
    ReusedPlan,
    Wardrobe(MaskWardrobeError),
}

impl MaskWardrobeControl {
    pub fn new(
        body_id: &BodyId,
        scoped_wardrobe: BodyMaskWardrobe,
        active_plan_id: PlanId,
        routes: &[SealedMaskRoute],
        selected: Option<SelectedMaskRoute>,
    ) -> Result<Self, MaskWardrobeControlError> {
        if scoped_wardrobe.body_id != *body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        let reconciliation = scoped_wardrobe
            .wardrobe
            .reconcile(&active_plan_id, routes, selected.as_ref())
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        Ok(Self {
            scoped_wardrobe,
            active_plan_id,
            selected: selection(&reconciliation.show),
        })
    }

    /// Apply one semantic user action. The action may select an already-sealed
    /// route, but it never mutates the immutable Plan or grants replanning.
    pub fn apply(
        &mut self,
        basis_revision: u64,
        action: MaskWardrobeAction,
        routes: &[SealedMaskRoute],
    ) -> Result<MaskWardrobeControlEvidence, MaskWardrobeControlError> {
        let next = match &action {
            MaskWardrobeAction::Wear(mask) => {
                self.scoped_wardrobe.wear(basis_revision, mask.clone())
            }
            MaskWardrobeAction::Doff(mask) => self.scoped_wardrobe.doff(basis_revision, mask),
            MaskWardrobeAction::Prefer(preference) => self
                .scoped_wardrobe
                .prefer(basis_revision, preference.clone()),
        }
        .map_err(MaskWardrobeControlError::Wardrobe)?;
        let reconciliation = next
            .wardrobe
            .reconcile(&self.active_plan_id, routes, self.selected.as_ref())
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        self.selected = selection(&reconciliation.show);
        self.scoped_wardrobe = next;
        Ok(MaskWardrobeControlEvidence {
            body_id: self.scoped_wardrobe.body_id.clone(),
            wake_id: self.scoped_wardrobe.wake_id.clone(),
            action,
            basis_revision,
            resulting_wardrobe: self.scoped_wardrobe.wardrobe.clone(),
            active_plan_id: self.active_plan_id.clone(),
            reconciliation,
        })
    }

    /// Admit an exact replacement Plan after an authorized planning owner has
    /// produced it. Discovery or availability alone cannot call this a replan.
    pub fn admit_replacement_plan(
        &mut self,
        basis_plan_id: &PlanId,
        replacement_plan_id: PlanId,
        routes: &[SealedMaskRoute],
    ) -> Result<MaskReconciliation, MaskWardrobeControlError> {
        if basis_plan_id != &self.active_plan_id {
            return Err(MaskWardrobeControlError::StalePlan);
        }
        if replacement_plan_id == self.active_plan_id {
            return Err(MaskWardrobeControlError::ReusedPlan);
        }
        let reconciliation = self
            .scoped_wardrobe
            .wardrobe
            .reconcile(&replacement_plan_id, routes, None)
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        self.active_plan_id = replacement_plan_id;
        self.selected = selection(&reconciliation.show);
        Ok(reconciliation)
    }
}

fn selection(disposition: &MaskShowDisposition) -> Option<SelectedMaskRoute> {
    match disposition {
        MaskShowDisposition::Retain(selected)
        | MaskShowDisposition::SelectSealed { selected, .. } => Some(selected.clone()),
        MaskShowDisposition::NoCurrentShow { .. } => None,
    }
}
