use super::*;
use std::sync::atomic::Ordering;

impl HostedCrecheAdapter for CrecheEncounterAdapter<'_> {
    fn resource_offer(&self) -> ResourceOffer {
        self.channel.inputs.resource.clone()
    }
    fn limits(&self) -> CrecheLimits {
        self.channel.inputs.installation.limits
    }
    fn prepare(
        &mut self,
        context: &CrecheExecutionContext,
    ) -> Result<PreparedCrecheRequest, String> {
        if self.request.is_some()
            || self.retired
            || context.resource_offer() != &self.resource_offer()
            || context.limits() != self.limits()
            || context.plan().body_id != self.channel.binding.biography.body_id
        {
            return Err(
                "Crèche preparation repeated or differs from installed Body/resource".into(),
            );
        }
        let partition = context
            .plan()
            .plots
            .iter()
            .find(|p| &p.plot == context.plot())
            .ok_or("Crèche exact producing Plot absent")?;
        let basis = HostConfigurationEncounterBasis {
            host_id: context.host().host_id.clone(),
            boot_id: context.host().boot_id.clone(),
            encounter_id: self.channel.inputs.installation.encounter_id.clone(),
            execution: FaceBasis {
                body_id: Some(context.plan().body_id.clone()),
                wake_id: Some(context.plan().wake_id.clone()),
                source_document_id: Some(partition.plan.source_document_id.clone()),
                checked_plot_id: Some(partition.plan.checked_plot_id.clone()),
                expanded_plot_id: Some(partition.plan.expanded_plot_id.clone()),
                plan_id: Some(partition.plan.plan_id.clone()),
                active_play_id: Some(context.play().active_play_id.clone()),
                sign_ids: vec![],
            },
        };
        let face = self.encounter.face(&basis)?;
        // This context already retains the exact biography; avoid duplicating
        // its potentially long lifecycle history merely to bind an operation.
        let digest = Sha256::digest(
            serde_json::to_vec(&(
                self.channel.inputs.admission_digest,
                &self.channel.binding.biography.body_id,
                &self.channel.binding.biography.body.identity_derivation,
                &self.channel.binding.invitation,
            ))
            .map_err(|e| e.to_string())?,
        )
        .into();
        let request = context.bind_request(digest)?;
        let mut state = self.channel.lock()?;
        state.face = Some(Arc::new(face));
        state.request = Some(request);
        self.basis = Some(basis);
        self.request = Some(request);
        Ok(PreparedCrecheRequest {
            admitted_state_digest: digest,
        })
    }
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        if self.request != Some(request) || self.begun || self.retired {
            return Err("Crèche begin stale or repeated".into());
        }
        let mut state = self
            .channel
            .state
            .try_lock()
            .map_err(|_| "Crèche mailbox occupied at begin")?;
        if state.face.is_none() {
            return Err("Crèche initial Face absent".into());
        }
        state.active = true;
        self.begun = true;
        Ok(CrechePoll::Pending)
    }
    fn poll(&mut self) -> Result<CrechePoll, String> {
        if !self.begun || self.retired {
            return Err("Crèche encounter inactive".into());
        }
        if let Some(outcome) = self.outcome {
            return Ok(CrechePoll::Outcome(outcome));
        }
        let channel = self.channel.clone();
        let mut state = match channel.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => return Ok(CrechePoll::Pending),
            Err(TryLockError::Poisoned(_)) => return Err("Crèche mailbox poisoned".into()),
        };
        if let Some(result) = state.result.take() {
            match result {
                Ok(products) => {
                    if products.receipt.len() as u64 > self.limits().maximum_evidence_bytes {
                        return Err("Crèche receipt exceeds retained evidence capacity".into());
                    }
                    self.evidence.clear();
                    self.evidence.extend_from_slice(&products.receipt);
                    let outcome = CrecheOutcome {
                        request: self.request.ok_or("Crèche producing request absent")?,
                        disposition: CrecheDisposition::Prepared,
                        evidence_digest: Sha256::digest(&self.evidence).into(),
                    };
                    state.products = Some(products);
                    state.published = true;
                    state.active = false;
                    state.acknowledged = None;
                    self.outcome = Some(outcome);
                    return Ok(CrechePoll::Outcome(outcome));
                }
                Err(reason) => {
                    drop(state);
                    return self.terminal(CrecheDisposition::Refused, &reason);
                }
            }
        }
        let Some(command) = state.command.take() else {
            return Ok(CrechePoll::Pending);
        };
        let face = state.face.as_ref().ok_or("Crèche current Face absent")?;
        let mut custody = match channel.custody.try_lock() {
            Ok(c) => c,
            Err(TryLockError::WouldBlock) => {
                state.command = Some(command);
                return Ok(CrechePoll::Pending);
            }
            Err(TryLockError::Poisoned(_)) => return Err("Crèche Mask custody poisoned".into()),
        };
        custody.validate_acknowledged(face, &command.show)?;
        custody.validate_returned_interaction(face, &command.show, &command.input)?;
        drop(custody);
        let basis = self.basis.as_ref().ok_or("Crèche execution basis absent")?;
        let result = self
            .encounter
            .apply_interaction(basis, &command.show, &command.input)?;
        state.acknowledged = None;
        match result {
            HostConfigurationEncounterOutcome::Changed => {
                state.face = Some(Arc::new(self.encounter.face(basis)?))
            }
            HostConfigurationEncounterOutcome::Checked(configuration) => {
                let mut face = self.encounter.face(basis)?;
                for action in &mut face.actions {
                    action.availability = conduit_face::FaceActionAvailability::Unavailable {
                        reason_code: "creche-seed-preparation-pending".into(),
                        explanation: "Checked Host intent; awaiting admitted Seed production"
                            .into(),
                    };
                }
                // Rebuild identity after changing action semantics.
                let face = Face::new_with_semantics(
                    face.revision,
                    face.basis,
                    face.subjects,
                    face.relationships,
                    face.properties,
                    face.text,
                    face.actions,
                    face.disclosures,
                )
                .map_err(|e| format!("Crèche pending Face: {e:?}"))?;
                state.face = Some(Arc::new(face));
                state.checked = Some(*configuration);
            }
        }
        Ok(CrechePoll::Pending)
    }
    fn retained_evidence(&self, outcome: &CrecheOutcome) -> Result<&[u8], String> {
        if self.outcome.as_ref() != Some(outcome) {
            return Err("Crèche outcome lacks exact retained receipt".into());
        }
        Ok(&self.evidence)
    }
    fn cancel_and_drain(&mut self) -> Result<(), String> {
        if self.retired {
            return Ok(());
        }
        let handle = {
            let mut state = self.channel.lock()?;
            state.active = false;
            state.command = None;
            state.acknowledged = None;
            state.cancelled.store(true, Ordering::Release);
            state.worker.take()
        };
        if let Some(handle) = handle {
            handle
                .join()
                .map_err(|_| "Crèche worker panicked while draining")?;
        }
        self.channel
            .custody
            .lock()
            .map_err(|_| "Crèche Mask custody poisoned")?
            .retire_and_drain()?;
        if self.request.is_some() && self.outcome.is_none() {
            self.terminal(CrecheDisposition::Cancelled, "cancelled")?;
        }
        self.channel.lock()?.retired = true;
        self.retired = true;
        Ok(())
    }
}
impl CrecheEncounterAdapter<'_> {
    fn terminal(
        &mut self,
        disposition: CrecheDisposition,
        reason: &str,
    ) -> Result<CrechePoll, String> {
        let mut end = reason.len().min(256);
        while !reason.is_char_boundary(end) {
            end -= 1;
        }
        let evidence=serde_json::to_vec(&serde_json::json!({"schema":"conduit.creche/terminal@1","request":self.request.ok_or("Crèche terminal request absent")?.0,"reason":&reason[..end]})).map_err(|e|e.to_string())?;
        if evidence.len() as u64 > self.limits().maximum_evidence_bytes {
            return Err("Crèche terminal evidence bound exhausted".into());
        }
        self.evidence.clear();
        self.evidence.extend_from_slice(&evidence);
        let outcome = CrecheOutcome {
            request: self.request.unwrap(),
            disposition,
            evidence_digest: Sha256::digest(&self.evidence).into(),
        };
        let mut state = self.channel.lock()?;
        state.active = false;
        state.acknowledged = None;
        self.outcome = Some(outcome);
        Ok(CrechePoll::Outcome(outcome))
    }
}
impl Drop for CrecheEncounterAdapter<'_> {
    fn drop(&mut self) {
        if !self.retired {
            let _ = self.cancel_and_drain();
        }
    }
}
