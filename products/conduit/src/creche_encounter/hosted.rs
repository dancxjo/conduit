//! Installed operation lifecycle; poll performs no file or thread work.
use super::*;

impl HostedCrecheAdapter for CrecheEncounterAdapter {
    fn resource_offer(&self) -> ResourceOffer {
        self.resource.clone()
    }
    fn limits(&self) -> CrecheLimits {
        self.limits
    }
    fn prepare(
        &mut self,
        context: &CrecheExecutionContext,
    ) -> Result<PreparedCrecheRequest, String> {
        if self.prepared
            || self.begun
            || self.retired
            || context.resource_offer() != &self.resource
            || context.limits() != self.limits
        {
            return Err("Crèche preparation is stale or repeated".into());
        }
        let basis = admission::basis(&self.encounter_id, context)?;
        let face = self
            .encounter
            .as_ref()
            .ok_or("Crèche draft missing")?
            .face(&basis)
            .map_err(|e| format!("Crèche Face preparation: {e:?}"))?;
        self.request = Some(context.bind_request(self.admission_digest)?);
        self.context = Some(context.clone());
        self.basis = Some(basis);
        self.channel.lock()?.face = Some(Arc::new(face));
        self.prepared = true;
        Ok(PreparedCrecheRequest {
            admitted_state_digest: self.admission_digest,
        })
    }
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        if !self.prepared || self.begun || self.retired || self.request != Some(request) {
            return Err("Crèche begin lacks its exact prepared request".into());
        }
        let mut state = self
            .channel
            .state
            .try_lock()
            .map_err(|_| "Crèche mailbox occupied at begin")?;
        if state.face.is_none() {
            return Err("Crèche initial Face was not prepared".into());
        }
        state.active = true;
        self.begun = true;
        Ok(CrechePoll::Pending)
    }
    fn poll(&mut self) -> Result<CrechePoll, String> {
        if !self.begun || self.retired {
            return Err("Crèche encounter is not active".into());
        }
        if let Some(outcome) = self.outcome {
            return Ok(CrechePoll::Outcome(outcome));
        }
        let channel = self.channel.clone();
        let mut state = match channel.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => return Ok(CrechePoll::Pending),
            Err(TryLockError::Poisoned(_)) => {
                return Err("Crèche encounter mailbox is poisoned".into())
            }
        };
        if let Some(reason) = state.make_refusal.take() {
            state.active = false;
            state.acknowledged = None;
            drop(state);
            return self.terminal(TerminalReason::Make(reason));
        }
        if let Some(evidence) = state
            .images
            .as_mut()
            .and_then(|images| images.evidence.take())
        {
            self.evidence = evidence;
            let outcome = CrecheOutcome {
                request: self
                    .request
                    .ok_or("Crèche image completion lacks request")?,
                disposition: CrecheDisposition::Prepared,
                evidence_digest: Sha256::digest(&self.evidence).into(),
            };
            state.products_completed = true;
            state.active = false;
            state.acknowledged = None;
            self.outcome = Some(outcome);
            return Ok(CrechePoll::Outcome(outcome));
        }
        let Some(command) = state.command.take() else {
            return Ok(CrechePoll::Pending);
        };
        if self.interactions >= self.maximum_interactions {
            state.active = false;
            state.acknowledged = None;
            drop(state);
            return self.terminal(TerminalReason::Make(CrecheMakeRefusal::BoundExhausted));
        }
        // The queued occurrence must still belong to the selected actual Mask
        // when the semantic owner consumes it, including renderer revocation.
        let custody = match channel.custody.try_lock() {
            Ok(custody) => custody,
            Err(TryLockError::WouldBlock) => {
                state.command = Some(command);
                return Ok(CrechePoll::Pending);
            }
            Err(TryLockError::Poisoned(_)) => return Err("Crèche Mask custody poisoned".into()),
        };
        let mut custody = custody;
        let current = state.face.as_ref().ok_or("Crèche current Face absent")?;
        let admitted = custody
            .validate_acknowledged(current, &command.show)
            .and_then(|()| {
                custody.validate_returned_interaction(current, &command.show, &command.interaction)
            });
        drop(custody);
        if admitted.is_err() {
            state.active = false;
            state.acknowledged = None;
            drop(state);
            return self.terminal(TerminalReason::InteractionRefused);
        }
        self.interactions += 1;
        if state.images.is_some() {
            if images_runtime::consume(&mut state, &command).is_err() {
                state.active = false;
                state.acknowledged = None;
                drop(state);
                return self.terminal(TerminalReason::InteractionRefused);
            }
            return Ok(CrechePoll::Pending);
        }

        let encounter = self
            .encounter
            .as_mut()
            .ok_or("Crèche review was already handed off")?;
        let basis = self
            .basis
            .as_ref()
            .ok_or("Crèche producing execution missing")?;
        let result = encounter.apply_interaction(
            basis,
            &command.show,
            &command.interaction,
            &self.catalog,
            &self.packages,
        );
        state.acknowledged = None;
        match result {
            Ok(DeploymentEncounterOutcome::Changed) => {
                state.face = Some(Arc::new(
                    encounter
                        .face(basis)
                        .map_err(|e| format!("Crèche changed Face: {e:?}"))?,
                ));
            }
            Ok(DeploymentEncounterOutcome::Reviewed(review)) => {
                preparing::publish(&mut state)?;
                state.reviewed = true;
                state.handoff = Some(CrecheReviewedHandoff {
                    request: self.request.unwrap(),
                    context: self
                        .context
                        .take()
                        .ok_or("Crèche handoff execution missing")?,
                    draft: self.encounter.take().unwrap().into_draft(),
                    review,
                });
                // No successful image outcome is inferred from canonical review.
            }
            Err(_) => {
                state.active = false;
                drop(state);
                return self.terminal(TerminalReason::InteractionRefused);
            }
        }
        Ok(CrechePoll::Pending)
    }
    fn retained_evidence(&self, outcome: &CrecheOutcome) -> Result<&[u8], String> {
        if self.outcome.as_ref() != Some(outcome) {
            return Err("Crèche outcome lacks retained canonical evidence".into());
        }
        Ok(&self.evidence)
    }
    fn cancel_and_drain(&mut self) -> Result<(), String> {
        if self.retired {
            return Ok(());
        }
        let worker = self.channel.revoke_and_drain_worker();
        let presentation = self
            .channel
            .custody
            .lock()
            .map_err(|_| "Crèche Mask custody poisoned".to_owned())
            .and_then(|mut custody| custody.retire_and_drain());
        worker.and(presentation)?;
        if self.request.is_some() && self.outcome.is_none() {
            self.terminal(TerminalReason::Cancelled)?;
        }
        self.channel.lock()?.retirement_complete = true;
        self.retired = true;
        Ok(())
    }
}
