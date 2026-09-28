//! Bounded ordinary body-wide planning and execution history.
//!
//! The session is orchestration, not a second planner or lifecycle. Callers
//! supply ordinary per-Form Plans; `BodyPlan` seals the exact workset and
//! `Wake` owns every accepted, superseded, playing, and unsatisfied state.

use crate::{
    Body, BodyFormPlan, BodyId, BodyLifecycleError, BodyMaskTopology, BodyPlan, BodyPlanError,
    BodyPlayIdentity, Wake, WakeId, WakeLifecycle,
};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{BootId, HostId, PlanId, SignId};
use serde::{Deserialize, Serialize};

mod continuity;
mod execution;
pub use execution::{BodyExecutionClaim, BodyExecutionClaimError, BodyExecutionPhase};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyPlanningTransition {
    pub unsatisfied_sign_id: Option<SignId>,
    pub plan_ready_sign_id: SignId,
    pub play_sequence: u64,
    pub play_started_sign_id: SignId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyPlanningSessionSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_claims: Vec<BodyExecutionClaim>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_proposal_sign_id: Option<SignId>,
    pub body_id: BodyId,
    pub wake_id: WakeId,
    pub lifecycle: WakeLifecycle,
    pub current_plan_id: PlanId,
    pub historical_plan_ids: Vec<PlanId>,
    pub current_hosts: Vec<BodyPlanningHost>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BodyPlanningHost {
    pub host_id: HostId,
    pub boot_id: BootId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyPlanningSessionError {
    Lifecycle(BodyLifecycleError),
    Plan(BodyPlanError),
    MissingUnsatisfiedSign,
    StaleCurrentPlan,
    OutstandingExecution,
    ExecutionTerminationAbsent,
    MissingForm,
    InvalidForm(String),
    Planning(String),
}

#[derive(Debug, Clone)]
pub struct BodyPlanningSession {
    execution_claims: Vec<BodyExecutionClaim>,
    unavailable_proposal_sign_id: Option<SignId>,
    body: Body,
    wake: Wake,
    plans: Vec<BodyPlan>,
}

impl BodyPlanningSession {
    /// Plan the current workset without claiming Host admission or Play start.
    pub fn prepare(
        body: &Body,
        wake_sequence: u64,
        wake_sign_id: SignId,
        forms: Vec<BodyFormPlan>,
    ) -> Result<Self, BodyPlanningSessionError> {
        Self::prepare_with_masks(body, wake_sequence, wake_sign_id, forms, Vec::new())
    }

    pub fn prepare_with_masks(
        body: &Body,
        wake_sequence: u64,
        wake_sign_id: SignId,
        forms: Vec<BodyFormPlan>,
        mask_topologies: Vec<BodyMaskTopology>,
    ) -> Result<Self, BodyPlanningSessionError> {
        let (body, wake) = body
            .wake(wake_sequence, wake_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let plan = BodyPlan::seal_with_masks(&wake, forms, mask_topologies)
            .map_err(BodyPlanningSessionError::Plan)?;
        Ok(Self {
            execution_claims: Vec::new(),
            body,
            wake,
            plans: vec![plan],
            unavailable_proposal_sign_id: None,
        })
    }

    /// Refresh an unstarted proposal. Retiring a running play needs a separate
    /// attributable lifecycle event; offer availability alone cannot do it.
    pub fn replace_proposal(
        &mut self,
        forms: Vec<BodyFormPlan>,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        let mask_topologies = self.current_plan().mask_topologies.clone();
        self.replace_proposal_with_masks(forms, mask_topologies)
    }

    pub fn replace_proposal_with_masks(
        &mut self,
        forms: Vec<BodyFormPlan>,
        mask_topologies: Vec<BodyMaskTopology>,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        if self.has_outstanding_execution_claim()
            || self.wake.lifecycle != WakeLifecycle::AwaitingPlan
            || !self.wake.plans.is_empty()
        {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        if self.plans.len() >= crate::MAX_WAKE_PLANS {
            return Err(BodyPlanningSessionError::Lifecycle(
                BodyLifecycleError::PlanCapacityExhausted,
            ));
        }
        let plan = BodyPlan::seal_with_masks(&self.wake, forms, mask_topologies)
            .map_err(BodyPlanningSessionError::Plan)?;
        if plan.plan_id == self.current_plan().plan_id {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        self.plans.push(plan);
        self.unavailable_proposal_sign_id = None;
        Ok(self.current_plan())
    }

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
        Self::start_with_masks(
            body,
            wake_sequence,
            wake_sign_id,
            forms,
            Vec::new(),
            plan_ready_sign_id,
            play_sequence,
            play_started_sign_id,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_with_masks(
        body: &Body,
        wake_sequence: u64,
        wake_sign_id: SignId,
        forms: Vec<BodyFormPlan>,
        mask_topologies: Vec<BodyMaskTopology>,
        plan_ready_sign_id: SignId,
        play_sequence: u64,
        play_started_sign_id: SignId,
    ) -> Result<Self, BodyPlanningSessionError> {
        let (body, wake) = body
            .wake(wake_sequence, wake_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let plan = BodyPlan::seal_with_masks(&wake, forms, mask_topologies)
            .map_err(BodyPlanningSessionError::Plan)?;
        let wake = wake
            .body_plan_ready(&plan, plan_ready_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let play = BodyPlayIdentity::bind(&plan, play_sequence);
        let wake = wake
            .body_play_started(&plan, &play, play_started_sign_id)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        Ok(Self {
            execution_claims: Vec::new(),
            body,
            wake,
            plans: vec![plan],
            unavailable_proposal_sign_id: None,
        })
    }

    pub fn replan(
        &mut self,
        forms: Vec<BodyFormPlan>,
        transition: BodyPlanningTransition,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        let mask_topologies = self.current_plan().mask_topologies.clone();
        self.replan_with_masks(forms, mask_topologies, transition)
    }

    pub fn replan_with_masks(
        &mut self,
        forms: Vec<BodyFormPlan>,
        mask_topologies: Vec<BodyMaskTopology>,
        transition: BodyPlanningTransition,
    ) -> Result<&BodyPlan, BodyPlanningSessionError> {
        if self.has_outstanding_execution_claim() {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
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
        let replacement = BodyPlan::seal_with_masks(&wake, forms, mask_topologies)
            .map_err(BodyPlanningSessionError::Plan)?;
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
        if self.wake.lifecycle == WakeLifecycle::AwaitingPlan {
            self.unavailable_proposal_sign_id = Some(sign_id);
            return Ok(self.current_plan());
        }
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
        self.plans.last().expect("a planning session has a plan")
    }

    pub fn plan(&self, plan_id: &PlanId) -> Option<&BodyPlan> {
        self.plans.iter().find(|plan| &plan.plan_id == plan_id)
    }

    pub fn snapshot(&self) -> BodyPlanningSessionSnapshot {
        let mut current_hosts = self
            .current_plan()
            .forms
            .iter()
            .flat_map(|form| &form.plan.fragments)
            .map(|fragment| BodyPlanningHost {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
            })
            .collect::<Vec<_>>();
        current_hosts.sort();
        current_hosts.dedup();
        BodyPlanningSessionSnapshot {
            execution_claims: self.execution_claims.clone(),
            unavailable_proposal_sign_id: self.unavailable_proposal_sign_id.clone(),
            body_id: self.body.body_id.clone(),
            wake_id: self.wake.wake_id.clone(),
            lifecycle: self.wake.lifecycle,
            current_plan_id: self.current_plan().plan_id.clone(),
            historical_plan_ids: self.plans.iter().map(|plan| plan.plan_id.clone()).collect(),
            current_hosts,
        }
    }
}
