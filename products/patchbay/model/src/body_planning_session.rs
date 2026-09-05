//! Bounded ordinary Body-wide planning history for the Patchbay product.
//!
//! The session is orchestration, not a second planner or lifecycle. Callers
//! supply ordinary per-Form Plans; `BodyPlan` seals the exact workset and
//! `Wake` owns every accepted, superseded, playing, and unsatisfied state.

use conduit_body::{
    Body, BodyFormPlan, BodyId, BodyLifecycleError, BodyPlan, BodyPlanError, BodyPlayIdentity,
    Wake, WakeId, WakeLifecycle,
};
use conduit_core::{PlanId, SignId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPlanningTransition {
    pub unsatisfied_sign_id: Option<SignId>,
    pub plan_ready_sign_id: SignId,
    pub play_sequence: u64,
    pub play_started_sign_id: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlanningSessionSnapshot {
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub lifecycle: WakeLifecycle,
    pub current_plan_id: PlanId,
    pub historical_plan_ids: Vec<PlanId>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyPlanningSessionError {
    Lifecycle(BodyLifecycleError),
    Plan(BodyPlanError),
    MissingUnsatisfiedSign,
    StaleCurrentPlan,
}

#[derive(Debug, Clone)]
pub struct BodyPlanningSession {
    body: Body,
    wake: Wake,
    plans: Vec<BodyPlan>,
}

impl BodyPlanningSession {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        body: &Body,
        wake_sequence: u64,
        wake_sign_id: SignId,
        forms: Vec<BodyFormPlan>,
        plan_ready_sign_id: SignId,
        play_sequence: u64,
        play_started_sign_id: SignId,
    ) -> Result<Self, BodyPlanningSessionError> {
        let (body, wake) = body
            .wake(wake_sequence, wake_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let plan = BodyPlan::seal(&wake, forms).map_err(BodyPlanningSessionError::Plan)?;
        let wake = wake
            .body_plan_ready(&plan, plan_ready_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let play = BodyPlayIdentity::bind(&plan, play_sequence);
        let wake = wake
            .body_play_started(&plan, &play, play_started_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        Ok(Self {
            body,
            wake,
            plans: vec![plan],
        })
    }

    pub fn replan(
        &mut self,
        forms: Vec<BodyFormPlan>,
        transition: BodyPlanningTransition,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        let mut wake = self.wake.clone();
        if wake.lifecycle == WakeLifecycle::Playing {
            let sign = transition
                .unsatisfied_sign_id
                .ok_or(BodyPlanningSessionError::MissingUnsatisfiedSign)?;
            wake = wake
                .became_unsatisfied(&self.current_plan().plan_id, sign)
                .map_err(BodyPlanningSessionError::Lifecycle)?;
        }
        if wake.lifecycle != WakeLifecycle::Unsatisfied {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        let replacement = BodyPlan::seal(&wake, forms).map_err(BodyPlanningSessionError::Plan)?;
        wake = wake
            .body_plan_ready(&replacement, transition.plan_ready_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let play = BodyPlayIdentity::bind(&replacement, transition.play_sequence);
        wake = wake
            .body_play_started(&replacement, &play, transition.play_started_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        self.wake = wake;
        self.plans.push(replacement);
        Ok(self.current_plan())
    }

    pub fn mark_current_unsatisfied(
        &mut self,
        sign_id: SignId,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        let plan_id = self.current_plan().plan_id.clone();
        self.wake = self
            .wake
            .became_unsatisfied(&plan_id, sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        Ok(self.current_plan())
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn wake(&self) -> &Wake {
        &self.wake
    }

    pub fn current_plan(&self) -> &BodyPlan {
        self.plans.last().expect("a planning session has a Plan")
    }

    pub fn plan(&self, plan_id: &PlanId) -> Option<&BodyPlan> {
        self.plans.iter().find(|plan| &plan.plan_id == plan_id)
    }

    pub fn snapshot(&self) -> BodyPlanningSessionSnapshot {
        BodyPlanningSessionSnapshot {
            body_id: self.body.body_id.clone(),
            wake_id: self.wake.wake_id.clone(),
            lifecycle: self.wake.lifecycle,
            current_plan_id: self.current_plan().plan_id.clone(),
            historical_plan_ids: self.plans.iter().map(|plan| plan.plan_id.clone()).collect(),
        }
    }
}

#[cfg(test)]
mod tests;
