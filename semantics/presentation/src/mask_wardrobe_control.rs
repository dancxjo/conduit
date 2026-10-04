//! One Body-scoped semantic action seam for ordinary Plots worn as Masks.

use alloc::vec::Vec;
use conduit_body::{BodyId, BodyPlan, WakeId};
use conduit_core::{PlanId, PlotIdentity};
use serde::{Deserialize, Serialize};

use crate::{
    AdmittedMaskPlotRoutes, BodyMaskWardrobe, MaskReconciliation, MaskShowDisposition,
    MaskWardrobe, MaskWardrobeError, SelectedMaskPlotRoute,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaskWardrobeAction {
    Wear(PlotIdentity),
    Doff(PlotIdentity),
    Prefer(Vec<PlotIdentity>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub selected: Option<SelectedMaskPlotRoute>,
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
        active_plan: &BodyPlan,
        routes: &AdmittedMaskPlotRoutes,
        selected: Option<SelectedMaskPlotRoute>,
    ) -> Result<Self, MaskWardrobeControlError> {
        if scoped_wardrobe.body_id != *body_id || active_plan.body_id != *body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        if routes.plan_id() != &active_plan.plan_id {
            return Err(MaskWardrobeControlError::StalePlan);
        }
        Self::new_from_admitted_routes(scoped_wardrobe, routes, selected)
    }

    /// The admitted route may be sealed by a workload BodyPlan or by an
    /// owner-issued presentation route while the workload Body is lulled.
    pub fn new_from_admitted_routes(
        scoped_wardrobe: BodyMaskWardrobe,
        routes: &AdmittedMaskPlotRoutes,
        selected: Option<SelectedMaskPlotRoute>,
    ) -> Result<Self, MaskWardrobeControlError> {
        if routes.body_id() != &scoped_wardrobe.body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        let reconciliation = scoped_wardrobe
            .wardrobe
            .reconcile(routes.plan_id(), routes.routes(), selected.as_ref())
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        Ok(Self {
            scoped_wardrobe,
            active_plan_id: routes.plan_id().clone(),
            selected: selection(&reconciliation.show),
        })
    }

    pub fn apply(
        &mut self,
        basis_revision: u64,
        action: MaskWardrobeAction,
        routes: &AdmittedMaskPlotRoutes,
    ) -> Result<MaskWardrobeControlEvidence, MaskWardrobeControlError> {
        if routes.body_id() != &self.scoped_wardrobe.body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        if routes.plan_id() != &self.active_plan_id {
            return Err(MaskWardrobeControlError::StalePlan);
        }
        let wardrobe = match &action {
            MaskWardrobeAction::Wear(mask) => self
                .scoped_wardrobe
                .wardrobe
                .wear(basis_revision, mask.clone()),
            MaskWardrobeAction::Doff(mask) => {
                self.scoped_wardrobe.wardrobe.doff(basis_revision, mask)
            }
            MaskWardrobeAction::Prefer(preference) => self
                .scoped_wardrobe
                .wardrobe
                .prefer(basis_revision, preference.clone()),
        }
        .map_err(MaskWardrobeControlError::Wardrobe)?;
        let reconciliation = wardrobe
            .reconcile(
                &self.active_plan_id,
                routes.routes(),
                self.selected.as_ref(),
            )
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        self.scoped_wardrobe.wardrobe = wardrobe.clone();
        self.selected = selection(&reconciliation.show);
        Ok(MaskWardrobeControlEvidence {
            body_id: self.scoped_wardrobe.body_id.clone(),
            wake_id: self.scoped_wardrobe.wake_id.clone(),
            action,
            basis_revision,
            resulting_wardrobe: wardrobe,
            active_plan_id: self.active_plan_id.clone(),
            reconciliation,
        })
    }

    /// Reconcile current route availability without changing wardrobe policy.
    pub fn reconcile_routes(
        &mut self,
        routes: &AdmittedMaskPlotRoutes,
    ) -> Result<MaskReconciliation, MaskWardrobeControlError> {
        if routes.body_id() != &self.scoped_wardrobe.body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        if routes.plan_id() != &self.active_plan_id {
            return Err(MaskWardrobeControlError::StalePlan);
        }
        let reconciliation = self
            .scoped_wardrobe
            .wardrobe
            .reconcile(
                &self.active_plan_id,
                routes.routes(),
                self.selected.as_ref(),
            )
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        self.selected = selection(&reconciliation.show);
        Ok(reconciliation)
    }

    pub fn admit_replacement_plan(
        &mut self,
        basis_plan_id: &PlanId,
        replacement_plan: &BodyPlan,
        routes: &AdmittedMaskPlotRoutes,
    ) -> Result<MaskReconciliation, MaskWardrobeControlError> {
        if basis_plan_id != &self.active_plan_id {
            return Err(MaskWardrobeControlError::StalePlan);
        }
        if replacement_plan.body_id != self.scoped_wardrobe.body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        if routes.body_id() != &replacement_plan.body_id {
            return Err(MaskWardrobeControlError::WrongBody);
        }
        if routes.plan_id() != &replacement_plan.plan_id {
            return Err(MaskWardrobeControlError::StalePlan);
        }
        if replacement_plan.plan_id == self.active_plan_id {
            return Err(MaskWardrobeControlError::ReusedPlan);
        }
        let reconciliation = self
            .scoped_wardrobe
            .wardrobe
            .reconcile(&replacement_plan.plan_id, routes.routes(), None)
            .map_err(MaskWardrobeControlError::Wardrobe)?;
        self.active_plan_id = replacement_plan.plan_id.clone();
        self.selected = selection(&reconciliation.show);
        Ok(reconciliation)
    }
}

fn selection(disposition: &MaskShowDisposition) -> Option<SelectedMaskPlotRoute> {
    match disposition {
        MaskShowDisposition::Retain(selected)
        | MaskShowDisposition::SelectSealed { selected, .. } => Some(selected.clone()),
        MaskShowDisposition::NoCurrentShow { .. } => None,
    }
}
