//! Exact DOM input correlation and delivery through the Mask interaction Fore.

use super::*;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserMaskInteraction {
    pub show_id: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub action_id: String,
    pub target: String,
    pub arguments: Vec<FaceInteractionArgument>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskInteractionReceipt {
    pub schema: &'static str,
    pub correlation: MaskInteractionCorrelation,
    pub semantic_action: PresentationAction,
}

impl BrowserMaskRuntime {
    pub fn interact(
        &mut self,
        proposed: &BrowserMaskInteraction,
    ) -> Result<BrowserMaskInteractionReceipt, String> {
        if self.show.show.lifecycle != ManifestationLifecycle::Available {
            return Err("browser Mask refuses interaction before its Show is available".into());
        }
        if proposed.show_id != self.show.show_id.as_str()
            || proposed.presentation_id != self.presentation.identity.as_str()
            || proposed.presentation_revision != self.presentation.revision
        {
            return Err("browser Mask interaction is stale or mismatched".into());
        }
        if self.interaction_receipt.is_some() {
            return Err("browser Mask interaction Fore is already terminal".into());
        }
        let interaction = FaceInteraction::new(
            &self.presentation,
            &self.show,
            &proposed.action_id,
            &proposed.target,
            proposed.arguments.clone(),
            proposed.sequence,
        )
        .map_err(|refusal| format!("browser Mask interaction refused: {refusal:?}"))?;
        let semantic_action = self
            .presentation
            .resolve_action(self.presentation.revision, &proposed.action_id)
            .map_err(|refusal| format!("browser Mask semantic action refused: {refusal:?}"))?
            .clone();
        let correlation = self
            .show
            .correlate_interaction(interaction.clone())
            .map_err(|error| format!("browser Mask interaction correlation: {error:?}"))?;
        let bytes = interaction.encode();
        if bytes.len() > self.interaction_boundary.byte_capacity as usize {
            return Err("browser Mask interaction exceeds its sealed Fore bound".into());
        }
        let value = self
            .scheduler
            .store_host_value(&bytes)
            .map_err(|error| format!("store browser Mask interaction: {error:?}"))?;
        let node = self
            .pending_interaction_node
            .take()
            .ok_or("browser Mask has no pending DOM interaction effect")?;
        let request = self
            .pending_interaction_request
            .take()
            .ok_or("browser Mask has no pending DOM interaction request")?;
        self.scheduler
            .complete_host_call(
                node,
                request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, MASK_BYTES)
                            .map_err(|_| "browser Mask interaction bound")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete browser DOM interaction: {error:?}"))?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(
                    self.interaction_boundary.endpoint,
                    self.interaction_boundary.cord,
                )
                .map_err(|error| format!("offer browser Mask interaction: {error:?}"))?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("emit browser Mask interaction: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => return Err(format!("browser Mask ended without interaction: {other:?}")),
            }
        };
        let observed = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read browser Mask interaction: {error:?}"))?;
        if observed != bytes.as_slice() {
            return Err("browser Mask emitted a different interaction".into());
        }
        self.scheduler
            .remote_egress_accept(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("accept browser Mask interaction: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("deliver browser Mask interaction: {error:?}"))?;
        execution::drive_mask_to_terminal(
            &mut self.scheduler,
            [&self.show_boundary, &self.interaction_boundary],
        )?;
        let receipt = BrowserMaskInteractionReceipt {
            schema: "conduit.browser/mask-interaction@1",
            correlation,
            semantic_action,
        };
        self.interaction_receipt = Some(receipt.clone());
        Ok(receipt)
    }
}
