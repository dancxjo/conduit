//! Hosted bounded storage around the portable supervision state machine.
//!
//! The handler itself remains an ordinary planned node/composite. This module
//! only moves its typed terminal-observation and decision values through the
//! exact plan-visible capacities.

use std::collections::VecDeque;

use conduit_core::{
    ActionUsage, DecisionOutcome, SupervisionAdmissionEvidence, SupervisionContract,
    SupervisionDecision, SupervisionEvidence, SupervisionHostProfile, SupervisionReason,
    SupervisionState, TerminalObservation, terminal_observations_correlate,
};

/// Hosted storage for one exact supervisor binding.
pub struct BoundedSupervisionRuntime<'a> {
    contract: SupervisionContract<'a>,
    profile: SupervisionHostProfile,
    state: SupervisionState,
    pending: VecDeque<TerminalObservation<'a>>,
    usages: Vec<ActionUsage<'a>>,
    evidence: Vec<SupervisionEvidence>,
}

impl<'a> BoundedSupervisionRuntime<'a> {
    pub fn new(
        contract: SupervisionContract<'a>,
        profile: SupervisionHostProfile,
    ) -> Result<Self, SupervisionReason> {
        contract.validate()?;
        let usages = contract
            .actions
            .iter()
            .map(|action| ActionUsage {
                kind: action.kind,
                target: action.target,
                uses: 0,
            })
            .collect();
        Ok(Self {
            contract,
            profile,
            state: SupervisionState::new(),
            pending: VecDeque::with_capacity(usize::from(contract.limits.maximum_in_flight)),
            usages,
            evidence: Vec::with_capacity(usize::from(contract.limits.maximum_evidence_events)),
        })
    }

    /// Admit one already-terminal runtime subject. Domain values and pre-run
    /// diagnostics never enter this method.
    pub fn submit_terminal(
        &mut self,
        observation: TerminalObservation<'a>,
    ) -> Result<SupervisionAdmissionEvidence, SupervisionReason> {
        if self.pending.len() >= usize::from(self.contract.limits.maximum_in_flight) {
            return Err(SupervisionReason::InFlightLimitReached);
        }
        if self
            .evidence
            .len()
            .checked_add(2)
            .is_none_or(|needed| needed > usize::from(self.contract.limits.maximum_evidence_events))
        {
            return Err(SupervisionReason::EvidenceBudgetExhausted);
        }
        let emitted = self.state.admit_observation(self.contract, observation)?;
        self.pending.push_back(observation);
        self.evidence.extend([emitted.observed, emitted.admitted]);
        Ok(emitted)
    }

    /// Peek the next typed input an ordinary handler node should consume.
    #[must_use]
    pub fn next_observation(&self) -> Option<TerminalObservation<'a>> {
        self.pending.front().copied()
    }

    /// Accept one typed handler decision and remove the correlated observation
    /// only after the portable state transition succeeds.
    pub fn submit_decision(
        &mut self,
        observation: TerminalObservation<'a>,
        decision: SupervisionDecision<'a>,
    ) -> Result<DecisionOutcome<'a>, SupervisionReason> {
        let Some(index) = self
            .pending
            .iter()
            .position(|pending| terminal_observations_correlate(*pending, observation))
        else {
            let rejected = self.state.record_rejection(
                self.contract,
                Some(decision),
                SupervisionReason::ObservationInvalid,
            )?;
            self.evidence.push(rejected);
            return Err(SupervisionReason::ObservationInvalid);
        };
        let admitted_observation = self.pending[index];
        if self
            .evidence
            .len()
            .checked_add(2)
            .is_none_or(|needed| needed > usize::from(self.contract.limits.maximum_evidence_events))
        {
            if self.evidence.len() < usize::from(self.contract.limits.maximum_evidence_events) {
                let rejected = self.state.record_rejection(
                    self.contract,
                    Some(decision),
                    SupervisionReason::EvidenceBudgetExhausted,
                )?;
                self.evidence.push(rejected);
            }
            return Err(SupervisionReason::EvidenceBudgetExhausted);
        }
        match self.state.apply_decision(
            self.contract,
            self.profile,
            admitted_observation,
            decision,
            &mut self.usages,
        ) {
            Ok(outcome) => {
                self.pending.remove(index);
                self.evidence
                    .extend([outcome.accepted, outcome.consequence]);
                Ok(outcome)
            }
            Err(reason) => {
                let rejected =
                    self.state
                        .record_rejection(self.contract, Some(decision), reason)?;
                self.evidence.push(rejected);
                Err(reason)
            }
        }
    }

    pub fn cancel(&mut self) -> Result<SupervisionEvidence, SupervisionReason> {
        let evidence = self.state.cancel(self.contract)?;
        self.pending.clear();
        self.evidence.push(evidence);
        Ok(evidence)
    }

    pub fn handler_failed(&mut self) -> Result<SupervisionEvidence, SupervisionReason> {
        let evidence = self.state.handler_failed(self.contract)?;
        self.pending.clear();
        self.evidence.push(evidence);
        Ok(evidence)
    }

    pub fn handler_timed_out(
        &mut self,
        now_tick: u64,
    ) -> Result<SupervisionEvidence, SupervisionReason> {
        let observation = self
            .pending
            .front()
            .copied()
            .ok_or(SupervisionReason::ObservationInvalid)?;
        let evidence = self
            .state
            .handler_timed_out(self.contract, observation, now_tick)?;
        self.pending.clear();
        self.evidence.push(evidence);
        Err(SupervisionReason::HandlerTimeout)
    }

    pub fn cleanup_failed(&mut self) -> Result<SupervisionEvidence, SupervisionReason> {
        let evidence = self.state.cleanup_failed(self.contract)?;
        self.pending.clear();
        self.evidence.push(evidence);
        Err(SupervisionReason::CleanupFailed)
    }

    #[must_use]
    pub fn evidence(&self) -> &[SupervisionEvidence] {
        &self.evidence
    }

    #[must_use]
    pub const fn state(&self) -> SupervisionState {
        self.state
    }
}
