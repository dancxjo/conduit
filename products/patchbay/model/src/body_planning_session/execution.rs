//! Cooperative, in-memory coordinator fencing for the current local proposal.
//! A claim is not resource admission, a started Play, or authenticated evidence.
//! Unknown start outcomes remain outstanding: elapsed time cannot release them.
use super::*;
use conduit_body::{RemoteProofClass, MAX_WAKE_PLANS};
use conduit_core::bind_sign;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyExecutionPhase {
    Claimed,
    Started,
    RefusedBeforeStart {
        reason: String,
    },
    Terminal {
        disposition: String,
        sign_id: SignId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyExecutionClaim {
    pub play: BodyPlayIdentity,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub proof_class: RemoteProofClass,
    pub started_reported: bool,
    pub phase: BodyExecutionPhase,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyExecutionClaimError {
    OutstandingClaim,
    StaleProposal,
    WrongHost,
    CapacityExhausted,
    UnknownClaim,
    WrongPhase,
    InvalidReport,
}

impl BodyPlanningSession {
    pub fn has_outstanding_execution_claim(&self) -> bool {
        self.execution_claims.iter().any(|claim| {
            matches!(
                claim.phase,
                BodyExecutionPhase::Claimed | BodyExecutionPhase::Started
            )
        })
    }

    /// The caller must separately establish current membership and acquire Host
    /// resources. This only fences competing attempts in this coordinator session.
    pub fn claim_execution(
        &mut self,
        plan_id: &PlanId,
        host_id: &HostId,
        boot_id: &BootId,
    ) -> Result<BodyExecutionClaim, BodyExecutionClaimError> {
        use BodyExecutionClaimError::*;
        if self.has_outstanding_execution_claim() {
            return Err(OutstandingClaim);
        }
        if self.wake.lifecycle != WakeLifecycle::AwaitingPlan
            || !self.wake.plans.is_empty()
            || self.unavailable_proposal_sign_id.is_some()
            || &self.current_plan().plan_id != plan_id
        {
            return Err(StaleProposal);
        }
        if self.current_plan().forms.iter().any(|form| {
            form.plan.fragments.len() != 1
                || form
                    .plan
                    .fragments
                    .iter()
                    .any(|fragment| &fragment.host_id != host_id || &fragment.boot_id != boot_id)
        }) {
            return Err(WrongHost);
        }
        if self.execution_claims.len() >= MAX_WAKE_PLANS {
            return Err(CapacityExhausted);
        }
        let claim = BodyExecutionClaim {
            play: BodyPlayIdentity::bind(
                self.current_plan(),
                self.execution_claims.len() as u64 + 1,
            ),
            host_id: host_id.clone(),
            boot_id: boot_id.clone(),
            proof_class: RemoteProofClass::SelfReported,
            started_reported: false,
            phase: BodyExecutionPhase::Claimed,
        };
        self.execution_claims.push(claim.clone());
        Ok(claim)
    }

    fn claim_index(&self, play: &BodyPlayIdentity) -> Result<usize, BodyExecutionClaimError> {
        self.execution_claims
            .iter()
            .position(|claim| &claim.play == play)
            .ok_or(BodyExecutionClaimError::UnknownClaim)
    }

    /// Accept only the exact Wake produced by ordinary browser Body start.
    /// A Host lost between claim and report still started; retain that fact and
    /// then apply the recorded loss, instead of fabricating a pre-start refusal.
    pub fn report_execution_started(
        &mut self,
        play: &BodyPlayIdentity,
        reported_wake: &Wake,
    ) -> Result<(), BodyExecutionClaimError> {
        use BodyExecutionClaimError::*;
        let index = self.claim_index(play)?;
        let claim = &self.execution_claims[index];
        if claim.phase != BodyExecutionPhase::Claimed {
            return Err(WrongPhase);
        }
        let sign = |sequence| {
            bind_sign(
                &claim.host_id,
                &claim.boot_id,
                Some(&play.active_play_id),
                sequence,
            )
            .sign_id
        };
        let started = self
            .wake
            .body_plan_ready(self.current_plan(), sign(0))
            .and_then(|wake| wake.body_play_started(self.current_plan(), play, sign(1)))
            .map_err(|_| InvalidReport)?;
        if &started != reported_wake {
            return Err(InvalidReport);
        }
        let current = if let Some(loss) = &self.unavailable_proposal_sign_id {
            started
                .became_unsatisfied(&play.plan_id, loss.clone())
                .map_err(|_| InvalidReport)?
        } else {
            started
        };
        self.wake = current;
        self.execution_claims[index].started_reported = true;
        self.execution_claims[index].phase = BodyExecutionPhase::Started;
        Ok(())
    }

    /// Only a known pre-start refusal releases a claim without terminal proof.
    /// Transport failure or an unreadable successful start is not such a refusal.
    pub fn report_execution_refused(
        &mut self,
        play: &BodyPlayIdentity,
        reason: &str,
    ) -> Result<(), BodyExecutionClaimError> {
        let index = self.claim_index(play)?;
        if self.execution_claims[index].phase != BodyExecutionPhase::Claimed {
            return Err(BodyExecutionClaimError::WrongPhase);
        }
        if reason.is_empty() || reason.len() > 256 {
            return Err(BodyExecutionClaimError::InvalidReport);
        }
        self.execution_claims[index].phase = BodyExecutionPhase::RefusedBeforeStart {
            reason: reason.into(),
        };
        Ok(())
    }

    /// Preserve termination separately from Wake Lull. An exact cancellation
    /// may retire an attempt whose successful start envelope was not received.
    pub fn report_execution_terminal(
        &mut self,
        play: &BodyPlayIdentity,
        disposition: &str,
        terminal_sign_id: &SignId,
    ) -> Result<(), BodyExecutionClaimError> {
        let index = self.claim_index(play)?;
        let claim = &self.execution_claims[index];
        if !matches!(
            claim.phase,
            BodyExecutionPhase::Claimed | BodyExecutionPhase::Started
        ) {
            return Err(BodyExecutionClaimError::WrongPhase);
        }
        if !matches!(disposition, "completed" | "cancelled" | "failed")
            || bind_sign(
                &claim.host_id,
                &claim.boot_id,
                Some(&play.active_play_id),
                2,
            )
            .sign_id
                != *terminal_sign_id
        {
            return Err(BodyExecutionClaimError::InvalidReport);
        }
        self.execution_claims[index].phase = BodyExecutionPhase::Terminal {
            disposition: disposition.into(),
            sign_id: terminal_sign_id.clone(),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests;
