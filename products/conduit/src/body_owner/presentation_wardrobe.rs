//! One owner-held Body-lifetime wardrobe for already sealed presentation routes.
//! This selects ordinary Mask Plans; it never creates a workload Wake or Play.
use conduit_body::BodyLifecycleSession;
use conduit_core::{PlanId, PlotIdentity};
use conduit_presentation::{
    BodyMaskWardrobe, CurrentOwnerPresentationRoute, MaskReconciliation, MaskShow, MaskWardrobe,
    MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeControlError, MaskWardrobeControlEvidence,
    MaskWardrobeError, MaskWardrobeLifetime, OwnerPresentationPlan, OwnerPresentationPlanError,
    Presentation, SelectedMaskPlotRoute,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnerPresentationWardrobeError {
    Plan(OwnerPresentationPlanError),
    Wardrobe(MaskWardrobeError),
    Control(MaskWardrobeControlError),
    ReusedPlan,
    RouteNotSelected,
    ShowNotAcknowledged,
    InvalidShow,
}

/// An acknowledged Show belongs to one selected child of the outer owner
/// presentation Plan. It cannot be revived by selecting that child again.
#[derive(Clone)]
struct AcknowledgedShow {
    selected: SelectedMaskPlotRoute,
    child_route_plan_id: PlanId,
    show: MaskShow,
}

pub(crate) struct OwnerPresentationWardrobe {
    plan: OwnerPresentationPlan,
    control: MaskWardrobeControl,
    acknowledged: Option<AcknowledgedShow>,
}

impl OwnerPresentationWardrobe {
    pub(crate) fn seal(
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
        worn: Vec<PlotIdentity>,
        preference: Vec<PlotIdentity>,
    ) -> Result<Self, OwnerPresentationWardrobeError> {
        let plan = OwnerPresentationPlan::seal_current(session, face, current)
            .map_err(OwnerPresentationWardrobeError::Plan)?;
        let routes = plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerPresentationWardrobeError::Plan)?;
        let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, worn, preference)
            .map_err(OwnerPresentationWardrobeError::Wardrobe)?;
        let scoped = BodyMaskWardrobe::new(plan.body_id.clone(), None, wardrobe)
            .map_err(OwnerPresentationWardrobeError::Wardrobe)?;
        let control = MaskWardrobeControl::new_from_admitted_routes(scoped, &routes, None)
            .map_err(OwnerPresentationWardrobeError::Control)?;
        Ok(Self {
            plan,
            control,
            acknowledged: None,
        })
    }

    pub(crate) fn plan(&self) -> &OwnerPresentationPlan {
        &self.plan
    }

    pub(crate) fn control(&self) -> &MaskWardrobeControl {
        &self.control
    }

    /// Recheck actual current offers and Lines. A missing witness makes its
    /// sealed route unavailable; a stale or unsealed witness is refused.
    pub(crate) fn reconcile(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
    ) -> Result<MaskReconciliation, OwnerPresentationWardrobeError> {
        let routes = self
            .plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerPresentationWardrobeError::Plan)?;
        let mut next = self.control.clone();
        let reconciliation = next
            .reconcile_routes(&routes)
            .map_err(OwnerPresentationWardrobeError::Control)?;
        self.retain_acknowledgement_for(next.selected.as_ref());
        self.control = next;
        Ok(reconciliation)
    }

    /// Only the owner changes Body-lifetime eligibility or preference. A
    /// revision mismatch or invalid route witness changes no wardrobe state.
    pub(crate) fn apply(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
        basis_revision: u64,
        action: MaskWardrobeAction,
    ) -> Result<MaskWardrobeControlEvidence, OwnerPresentationWardrobeError> {
        let routes = self
            .plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerPresentationWardrobeError::Plan)?;
        let mut next = self.control.clone();
        let evidence = next
            .apply(basis_revision, action, &routes)
            .map_err(OwnerPresentationWardrobeError::Control)?;
        self.retain_acknowledgement_for(next.selected.as_ref());
        self.control = next;
        Ok(evidence)
    }

    /// A new Face, workload revision, offer, or Line needs a new immutable
    /// owner Plan. Preserve authored wardrobe policy, not its old selection or
    /// Show acknowledgement.
    pub(crate) fn replace(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
    ) -> Result<MaskReconciliation, OwnerPresentationWardrobeError> {
        let plan = OwnerPresentationPlan::seal_current(session, face, current)
            .map_err(OwnerPresentationWardrobeError::Plan)?;
        if plan.plan_id == self.plan.plan_id {
            return Err(OwnerPresentationWardrobeError::ReusedPlan);
        }
        let routes = plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerPresentationWardrobeError::Plan)?;
        let control = MaskWardrobeControl::new_from_admitted_routes(
            self.control.scoped_wardrobe.clone(),
            &routes,
            None,
        )
        .map_err(OwnerPresentationWardrobeError::Control)?;
        let reconciliation = control
            .scoped_wardrobe
            .wardrobe
            .reconcile(&plan.plan_id, routes.routes(), None)
            .map_err(OwnerPresentationWardrobeError::Wardrobe)?;
        self.plan = plan;
        self.control = control;
        self.acknowledged = None;
        Ok(reconciliation)
    }

    /// Reuse an unchanged outer Plan; otherwise replace it explicitly on a
    /// fresh Face, Host offer, or child route. Policy survives replacement.
    pub(crate) fn admit_or_replace(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
    ) -> Result<MaskReconciliation, OwnerPresentationWardrobeError> {
        match self.plan.admit_current_routes(session, face, current) {
            // A missing witness withdraws availability from its existing
            // route. It does not silently replace the immutable outer Plan.
            Ok(_) => self.reconcile(session, face, current),
            Err(OwnerPresentationPlanError::UnknownOrDuplicateWitness)
            | Err(OwnerPresentationPlanError::StaleBodyOrFace) => {
                self.replace(session, face, current)
            }
            Err(error) => Err(OwnerPresentationWardrobeError::Plan(error)),
        }
    }

    /// Record a real Mask Show only for the currently selected child route.
    /// The ordinary seal validates Face, Host, Boot, offer, Lines, and Show.
    pub(crate) fn acknowledge_selected_show(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
        child_route_plan_id: &PlanId,
        show: &MaskShow,
    ) -> Result<(), OwnerPresentationWardrobeError> {
        self.reconcile(session, face, current)?;
        let selected = self
            .control
            .selected
            .as_ref()
            .ok_or(OwnerPresentationWardrobeError::RouteNotSelected)?;
        let expected = format!("route/{}", child_route_plan_id.as_str());
        if selected.route_id != expected || selected.mask_plot != show.mask_plot {
            return Err(OwnerPresentationWardrobeError::RouteNotSelected);
        }
        let witness = current
            .iter()
            .find(|witness| route_plan_id(witness) == child_route_plan_id)
            .ok_or(OwnerPresentationWardrobeError::RouteNotSelected)?;
        validate_show(witness, session, face, show)?;
        self.acknowledged = Some(AcknowledgedShow {
            selected: selected.clone(),
            child_route_plan_id: child_route_plan_id.clone(),
            show: show.clone(),
        });
        Ok(())
    }

    /// Call immediately before owner action acceptance. An old Show cannot
    /// authorize an interaction after fallback, doff, or replacement.
    pub(crate) fn selected_show(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Presentation,
        current: &[CurrentOwnerPresentationRoute<'_>],
    ) -> Result<&MaskShow, OwnerPresentationWardrobeError> {
        self.reconcile(session, face, current)?;
        let acknowledged = self
            .acknowledged
            .as_ref()
            .ok_or(OwnerPresentationWardrobeError::ShowNotAcknowledged)?;
        let witness = current
            .iter()
            .find(|witness| route_plan_id(witness) == &acknowledged.child_route_plan_id)
            .ok_or(OwnerPresentationWardrobeError::ShowNotAcknowledged)?;
        validate_show(witness, session, face, &acknowledged.show)?;
        Ok(&acknowledged.show)
    }

    fn retain_acknowledgement_for(&mut self, selected: Option<&SelectedMaskPlotRoute>) {
        if self
            .acknowledged
            .as_ref()
            .is_some_and(|ack| Some(&ack.selected) != selected)
        {
            self.acknowledged = None;
        }
    }

    pub(crate) fn forget_show_for(&mut self, child_route_plan_id: &PlanId) {
        if self
            .acknowledged
            .as_ref()
            .is_some_and(|ack| &ack.child_route_plan_id == child_route_plan_id)
        {
            self.acknowledged = None;
        }
        if self.control.selected.as_ref().is_some_and(|selected| {
            selected.route_id == format!("route/{}", child_route_plan_id.as_str())
        }) {
            // The provider was lost. Revocation does not change authored
            // wardrobe policy or make any other route available.
            self.control.selected = None;
        }
    }
}

fn route_plan_id<'a>(witness: &'a CurrentOwnerPresentationRoute<'_>) -> &'a PlanId {
    match witness {
        CurrentOwnerPresentationRoute::Local { seal, .. } => &seal.route_plan_id,
        CurrentOwnerPresentationRoute::Remote { seal, .. } => &seal.route_plan_id,
    }
}

fn validate_show(
    witness: &CurrentOwnerPresentationRoute<'_>,
    session: &BodyLifecycleSession,
    face: &Presentation,
    show: &MaskShow,
) -> Result<(), OwnerPresentationWardrobeError> {
    match witness {
        CurrentOwnerPresentationRoute::Local { seal, owner_offer } => seal
            .validate_available_show(session, face, owner_offer, show)
            .map_err(|_| OwnerPresentationWardrobeError::InvalidShow),
        CurrentOwnerPresentationRoute::Remote {
            seal,
            owner_offer,
            mask_host_offer,
            face_line,
            return_line,
            interaction_line,
        } => match interaction_line {
            Some(interaction_line) => seal.validate_available_show_with_interaction(
                session,
                face,
                owner_offer,
                mask_host_offer,
                face_line,
                return_line,
                interaction_line,
                show,
            ),
            None => seal.validate_available_show(
                session,
                face,
                owner_offer,
                mask_host_offer,
                face_line,
                return_line,
                show,
            ),
        }
        .map_err(|_| OwnerPresentationWardrobeError::InvalidShow),
    }
}
