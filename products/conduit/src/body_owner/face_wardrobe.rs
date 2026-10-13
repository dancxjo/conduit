//! One owner-held Body-lifetime wardrobe for already sealed face routes.
//! This selects ordinary Mask Plans; it never creates a workload Wake or Play.
use conduit_body::BodyLifecycleSession;
use conduit_core::{PlanId, PlotIdentity};
use conduit_face::{
    BodyMaskWardrobe, CurrentOwnerFaceRoute, Face, MaskReconciliation, MaskShow, MaskWardrobe,
    MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeControlError, MaskWardrobeControlEvidence,
    MaskWardrobeError, MaskWardrobeLifetime, OwnerFacePlan, OwnerFacePlanError,
    SelectedMaskPlotRoute,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnerFaceWardrobeError {
    Plan(OwnerFacePlanError),
    Wardrobe(MaskWardrobeError),
    Control(MaskWardrobeControlError),
    ReusedPlan,
    RouteNotSelected,
    ShowNotAcknowledged,
    InvalidShow,
}

/// An acknowledged Show belongs to one selected child of the outer owner
/// face Plan. It cannot be revived by selecting that child again.
#[derive(Clone)]
struct AcknowledgedShow {
    selected: SelectedMaskPlotRoute,
    child_route_plan_id: PlanId,
    show: MaskShow,
}

#[derive(Clone)]
pub(crate) struct OwnerFaceWardrobe {
    plan: OwnerFacePlan,
    control: MaskWardrobeControl,
    acknowledged: Option<AcknowledgedShow>,
}

impl OwnerFaceWardrobe {
    pub(crate) fn seal(
        session: &BodyLifecycleSession,
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
        worn: Vec<PlotIdentity>,
        preference: Vec<PlotIdentity>,
    ) -> Result<Self, OwnerFaceWardrobeError> {
        let plan = OwnerFacePlan::seal_current(session, face, current)
            .map_err(OwnerFaceWardrobeError::Plan)?;
        let routes = plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerFaceWardrobeError::Plan)?;
        let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, worn, preference)
            .map_err(OwnerFaceWardrobeError::Wardrobe)?;
        let scoped = BodyMaskWardrobe::new(plan.body_id.clone(), None, wardrobe)
            .map_err(OwnerFaceWardrobeError::Wardrobe)?;
        let control = MaskWardrobeControl::new_from_admitted_routes(scoped, &routes, None)
            .map_err(OwnerFaceWardrobeError::Control)?;
        Ok(Self {
            plan,
            control,
            acknowledged: None,
        })
    }

    pub(crate) fn plan(&self) -> &OwnerFacePlan {
        &self.plan
    }

    pub(crate) fn control(&self) -> &MaskWardrobeControl {
        &self.control
    }

    /// Explicit owner choice of one already sealed child. Preference alone
    /// retains a still-current prior Show, so release that selection through
    /// wardrobe actions and restore its worn eligibility after the new child
    /// is selected. All revisions advance in this one owner control turn.
    pub(crate) fn select_plot(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
        target: &PlotIdentity,
        route_plan_id: &PlanId,
    ) -> Result<(), OwnerFaceWardrobeError> {
        let previous = self
            .control
            .selected
            .as_ref()
            .map(|route| route.mask_plot.clone());
        let mut preference = self.control.scoped_wardrobe.wardrobe.preference.clone();
        if !self.control.scoped_wardrobe.wardrobe.worn.contains(target) {
            let revision = self.control.scoped_wardrobe.wardrobe.revision;
            self.apply(
                session,
                face,
                current,
                revision,
                MaskWardrobeAction::Wear(target.clone()),
            )?;
        }
        preference.retain(|plot| plot != target);
        preference.insert(0, target.clone());
        let revision = self.control.scoped_wardrobe.wardrobe.revision;
        self.apply(
            session,
            face,
            current,
            revision,
            MaskWardrobeAction::Prefer(preference),
        )?;
        if let Some(previous) = previous.filter(|plot| plot != target) {
            let revision = self.control.scoped_wardrobe.wardrobe.revision;
            self.apply(
                session,
                face,
                current,
                revision,
                MaskWardrobeAction::Doff(previous.clone()),
            )?;
            let revision = self.control.scoped_wardrobe.wardrobe.revision;
            self.apply(
                session,
                face,
                current,
                revision,
                MaskWardrobeAction::Wear(previous),
            )?;
        }
        if self.control.selected.as_ref().is_none_or(|selected| {
            selected.route_id != format!("route/{}", route_plan_id.as_str())
                || &selected.mask_plot != target
        }) {
            return Err(OwnerFaceWardrobeError::RouteNotSelected);
        }
        Ok(())
    }

    /// Recheck actual current offers and Lines. A missing witness makes its
    /// sealed route unavailable; a stale or unsealed witness is refused.
    pub(crate) fn reconcile(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
    ) -> Result<MaskReconciliation, OwnerFaceWardrobeError> {
        let routes = self
            .plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerFaceWardrobeError::Plan)?;
        let mut next = self.control.clone();
        let reconciliation = next
            .reconcile_routes(&routes)
            .map_err(OwnerFaceWardrobeError::Control)?;
        self.retain_acknowledgement_for(next.selected.as_ref());
        self.control = next;
        Ok(reconciliation)
    }

    /// Only the owner changes Body-lifetime eligibility or preference. A
    /// revision mismatch or invalid route witness changes no wardrobe state.
    pub(crate) fn apply(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
        basis_revision: u64,
        action: MaskWardrobeAction,
    ) -> Result<MaskWardrobeControlEvidence, OwnerFaceWardrobeError> {
        let routes = self
            .plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerFaceWardrobeError::Plan)?;
        let mut next = self.control.clone();
        let evidence = next
            .apply(basis_revision, action, &routes)
            .map_err(OwnerFaceWardrobeError::Control)?;
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
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
    ) -> Result<MaskReconciliation, OwnerFaceWardrobeError> {
        let plan = OwnerFacePlan::seal_current(session, face, current)
            .map_err(OwnerFaceWardrobeError::Plan)?;
        if plan.plan_id == self.plan.plan_id {
            return Err(OwnerFaceWardrobeError::ReusedPlan);
        }
        let routes = plan
            .admit_current_routes(session, face, current)
            .map_err(OwnerFaceWardrobeError::Plan)?;
        let control = MaskWardrobeControl::new_from_admitted_routes(
            self.control.scoped_wardrobe.clone(),
            &routes,
            None,
        )
        .map_err(OwnerFaceWardrobeError::Control)?;
        let reconciliation = control
            .scoped_wardrobe
            .wardrobe
            .reconcile(&plan.plan_id, routes.routes(), None)
            .map_err(OwnerFaceWardrobeError::Wardrobe)?;
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
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
    ) -> Result<MaskReconciliation, OwnerFaceWardrobeError> {
        match self.plan.admit_current_routes(session, face, current) {
            // A missing witness withdraws availability from its existing
            // route. It does not silently replace the immutable outer Plan.
            Ok(_) => self.reconcile(session, face, current),
            Err(OwnerFacePlanError::UnknownOrDuplicateWitness)
            | Err(OwnerFacePlanError::StaleBodyOrFace) => self.replace(session, face, current),
            Err(error) => Err(OwnerFaceWardrobeError::Plan(error)),
        }
    }

    /// Record a real Mask Show only for the currently selected child route.
    /// The ordinary seal validates Face, Host, Boot, offer, Lines, and Show.
    pub(crate) fn acknowledge_selected_show(
        &mut self,
        session: &BodyLifecycleSession,
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
        child_route_plan_id: &PlanId,
        show: &MaskShow,
    ) -> Result<(), OwnerFaceWardrobeError> {
        self.reconcile(session, face, current)?;
        let selected = self
            .control
            .selected
            .as_ref()
            .ok_or(OwnerFaceWardrobeError::RouteNotSelected)?;
        let expected = format!("route/{}", child_route_plan_id.as_str());
        if selected.route_id != expected || selected.mask_plot != show.mask_plot {
            return Err(OwnerFaceWardrobeError::RouteNotSelected);
        }
        let witness = current
            .iter()
            .find(|witness| route_plan_id(witness) == child_route_plan_id)
            .ok_or(OwnerFaceWardrobeError::RouteNotSelected)?;
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
        face: &Face,
        current: &[CurrentOwnerFaceRoute<'_>],
    ) -> Result<&MaskShow, OwnerFaceWardrobeError> {
        self.reconcile(session, face, current)?;
        let acknowledged = self
            .acknowledged
            .as_ref()
            .ok_or(OwnerFaceWardrobeError::ShowNotAcknowledged)?;
        let witness = current
            .iter()
            .find(|witness| route_plan_id(witness) == &acknowledged.child_route_plan_id)
            .ok_or(OwnerFaceWardrobeError::ShowNotAcknowledged)?;
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

fn route_plan_id<'a>(witness: &'a CurrentOwnerFaceRoute<'_>) -> &'a PlanId {
    match witness {
        CurrentOwnerFaceRoute::Local { seal, .. } => &seal.route_plan_id,
        CurrentOwnerFaceRoute::Remote { seal, .. } => &seal.route_plan_id,
    }
}

fn validate_show(
    witness: &CurrentOwnerFaceRoute<'_>,
    session: &BodyLifecycleSession,
    face: &Face,
    show: &MaskShow,
) -> Result<(), OwnerFaceWardrobeError> {
    match witness {
        CurrentOwnerFaceRoute::Local { seal, owner_offer } => seal
            .validate_available_show(session, face, owner_offer, show)
            .map_err(|_| OwnerFaceWardrobeError::InvalidShow),
        CurrentOwnerFaceRoute::Remote {
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
        .map_err(|_| OwnerFaceWardrobeError::InvalidShow),
    }
}
