//! Bounded Body workload changes and their Body-wide Plan/Play transitions.

use conduit_core::SignId;

use super::{
    validate_new_sign, Body, BodyLifecycleError, BodyState, Wake, WakeLifecycle, WakePlanState,
    MAX_BODY_SIGNS,
};
use crate::{
    BodyLifecycleEvent, BodyPlan, BodyPlanError, BodyPlayIdentity, BodyWorkset, ResidentForm,
    WakeLifecycleEvent,
};

impl Body {
    pub fn effective_workset(&self) -> Result<BodyWorkset, BodyLifecycleError> {
        self.workset.validate()?;
        Ok(self.workset.clone())
    }

    pub fn admit_form(
        &self,
        form: ResidentForm,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        validate_new_sign(&self.sign_ids, &sign_id, MAX_BODY_SIGNS)?;
        let mut next = self.clone();
        next.workset = self.effective_workset()?;
        next.workset.add(form.clone())?;
        next.workload_revision = self
            .workload_revision
            .checked_add(1)
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        next.sign_ids.push(sign_id.clone());
        next.events.push(BodyLifecycleEvent::FormAdmitted {
            source_document_id: form.source_document_id,
            checked_form_id: form.checked_form_id,
            workload_revision: next.workload_revision,
            sign_id,
        });
        next.validate()?;
        Ok(next)
    }

    pub fn remove_form(
        &self,
        form: &ResidentForm,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        validate_new_sign(&self.sign_ids, &sign_id, MAX_BODY_SIGNS)?;
        let mut next = self.clone();
        next.workset = self.effective_workset()?;
        next.workset.remove(form)?;
        next.workload_revision = self
            .workload_revision
            .checked_add(1)
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        next.sign_ids.push(sign_id.clone());
        next.events.push(BodyLifecycleEvent::FormRemoved {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            workload_revision: next.workload_revision,
            sign_id,
        });
        next.validate()?;
        Ok(next)
    }
}

impl Wake {
    pub fn body_plan_ready(
        &self,
        plan: &BodyPlan,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        plan.validate_for(self).map_err(|error| match error {
            BodyPlanError::StaleWorkload | BodyPlanError::WrongBody | BodyPlanError::WrongWake => {
                BodyLifecycleError::StalePlan
            }
            _ => BodyLifecycleError::InvalidPlan,
        })?;
        self.accept_plan_id(&plan.plan_id, sign_id)
    }

    pub fn body_play_started(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        plan.validate_for(self)
            .map_err(|_| BodyLifecycleError::StalePlan)?;
        if !play.validate_for(plan) {
            return Err(BodyLifecycleError::StalePlay);
        }
        self.start_play_identity(&play.plan_id, &play.active_play_id, sign_id)
    }

    /// Retires the active Body-wide Plan after the owning Body's exact Form
    /// workset changes. The Wake survives and must receive one replacement
    /// Body-wide Plan before another Play can start.
    pub fn workload_changed(
        &self,
        body: &Body,
        sign_id: SignId,
    ) -> Result<Self, BodyLifecycleError> {
        self.validate()?;
        body.validate()?;
        if self.lifecycle != WakeLifecycle::Playing
            || self.body_id != body.body_id
            || !matches!(&body.state, BodyState::Awake { wake_id } if wake_id == &self.wake_id)
        {
            return Err(BodyLifecycleError::MismatchedWake);
        }
        let replacement_workset = body.effective_workset()?;
        let replacement_revision = body.workload_revision;
        if replacement_revision <= self.workload_revision || replacement_workset == self.workset {
            return Err(BodyLifecycleError::InvalidTransition);
        }
        let prior_plan_id = self
            .plans
            .last()
            .map(|plan| plan.plan_id.clone())
            .ok_or(BodyLifecycleError::InvalidTransition)?;
        let mut next = self.clone();
        next.push_event(WakeLifecycleEvent::WorkloadChanged {
            prior_plan_id,
            prior_workload_revision: self.workload_revision,
            prior_workset: self.workset.clone(),
            replacement_workload_revision: replacement_revision,
            replacement_workset: replacement_workset.clone(),
            sign_id,
        })?;
        next.plans
            .last_mut()
            .ok_or(BodyLifecycleError::InvalidTransition)?
            .state = WakePlanState::Unsatisfied;
        next.workload_revision = replacement_revision;
        next.workset = replacement_workset;
        next.lifecycle = WakeLifecycle::Unsatisfied;
        next.validate()?;
        Ok(next)
    }
}
