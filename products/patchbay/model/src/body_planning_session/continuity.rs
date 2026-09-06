use super::*;

impl BodyPlanningSession {
    /// Lull is explicit and may not erase an outstanding or unaccounted Play.
    pub fn lull(
        &mut self,
        wake_sign: SignId,
        retained_sign: SignId,
    ) -> Result<(), BodyPlanningSessionError> {
        if self.has_outstanding_execution_claim() {
            return Err(BodyPlanningSessionError::OutstandingExecution);
        }
        if self
            .wake
            .plans
            .iter()
            .filter_map(|plan| plan.active_play_id.as_ref())
            .any(|id| {
                !self.execution_claims.iter().any(|claim| {
                    &claim.play.active_play_id == id
                        && matches!(claim.phase, BodyExecutionPhase::Terminal { .. })
                })
            })
        {
            return Err(BodyPlanningSessionError::ExecutionTerminationAbsent);
        }
        let wake = self
            .wake
            .lull(wake_sign)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        let body = self
            .body
            .retain_after_lull(&wake, retained_sign)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        self.wake = wake;
        self.body = body;
        Ok(())
    }

    /// Keep prior Plans and execution claims while waking the current workload.
    /// The same finite session budget spans all Wakes; sequences are not reused.
    pub fn prepare_next_wake(
        &mut self,
        body: &Body,
        sequence: u64,
        sign: SignId,
        forms: Vec<BodyFormPlan>,
    ) -> Result<(), BodyPlanningSessionError> {
        if self.has_outstanding_execution_claim()
            || self.body.state != conduit_body::BodyState::Lulled
            || self.wake.lifecycle != WakeLifecycle::Lulled
            || body.body_id != self.body.body_id
            || !body.events.starts_with(&self.body.events)
        {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        if self.plans.len() >= conduit_body::MAX_WAKE_PLANS {
            return Err(BodyPlanningSessionError::Lifecycle(
                BodyLifecycleError::PlanCapacityExhausted,
            ));
        }
        let (body, wake) = body
            .wake(sequence, sign)
            .map_err(BodyPlanningSessionError::Lifecycle)?;
        if self.body.events.iter().any(|event| {
            matches!(event,
            conduit_body::BodyLifecycleEvent::Woke { wake_id, .. } if wake_id == &wake.wake_id)
        }) {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        let plan = BodyPlan::seal(&wake, forms).map_err(BodyPlanningSessionError::Plan)?;
        if self.plans.iter().any(|prior| prior.plan_id == plan.plan_id) {
            return Err(BodyPlanningSessionError::StaleCurrentPlan);
        }
        self.plans.push(plan);
        self.body = body;
        self.wake = wake;
        self.unavailable_proposal_sign_id = None;
        Ok(())
    }
}
