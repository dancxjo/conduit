//! Typed interaction emission through the checked browser Mask Fore.

use super::*;

impl OwnerBrowserMask {
    pub(super) fn interact(
        &mut self,
        proposed: ProposedInteraction,
    ) -> Result<InteractionEmission<'_>, String> {
        if !self.interactions_admitted
            || self.show.show.lifecycle != ManifestationLifecycle::Available
        {
            return Err("owner interaction return is not admitted for this Show".into());
        }
        if proposed.show_id != self.show.show_id.as_str()
            || proposed.face_id != self.presentation.identity.as_str()
            || proposed.face_revision != self.presentation.revision.to_string()
        {
            return Err("owner browser interaction has a stale Face or Show".into());
        }
        let action = self
            .presentation
            .resolve_action(self.presentation.revision, &proposed.action_id)
            .map_err(|error| format!("owner browser action refused: {error:?}"))?;
        if action.target != proposed.target || action.arguments.len() != proposed.arguments.len() {
            return Err(
                "owner browser interaction has a different target or argument contract".into(),
            );
        }
        let arguments = proposed
            .arguments
            .into_iter()
            .map(|argument| {
                let declaration = action
                    .arguments
                    .iter()
                    .find(|declared| declared.name == argument.name)
                    .ok_or("owner browser interaction named an undeclared argument")?;
                Ok(FaceInteractionArgument {
                    name: argument.name,
                    value_kind: declaration.contract.value_kind.as_str().into(),
                    value: argument.value.into_bytes(),
                })
            })
            .collect::<Result<Vec<_>, &str>>()?;
        let interaction = FaceInteraction::new(
            &self.presentation,
            &self.show,
            &proposed.action_id,
            &proposed.target,
            arguments,
            proposed.sequence,
        )
        .map_err(|error| format!("owner browser interaction refused: {error:?}"))?;
        let bytes = interaction.encode();
        if bytes.len() > self.interaction_boundary.byte_capacity as usize {
            return Err("owner browser interaction exceeds its sealed Fore bound".into());
        }
        let value = self
            .scheduler
            .store_host_value(&bytes)
            .map_err(|error| format!("store owner browser interaction: {error:?}"))?;
        let (node, request) = self
            .pending_interaction
            .take()
            .ok_or("owner browser Mask interaction Fore is terminal")?;
        self.scheduler
            .complete_host_call(
                node,
                request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, MASK_BYTES).map_err(|_| "interaction bound")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete owner browser interaction: {error:?}"))?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(
                    self.interaction_boundary.endpoint,
                    self.interaction_boundary.cord,
                )
                .map_err(|error| format!("offer owner browser interaction: {error:?}"))?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("advance owner browser Mask: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "owner browser Mask ended without interaction: {other:?}"
                    ))
                }
            }
        };
        if self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read interaction: {error:?}"))?
            != bytes.as_slice()
        {
            return Err("owner browser Mask emitted a different interaction".into());
        }
        self.scheduler
            .remote_egress_accept(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("accept interaction: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("deliver interaction: {error:?}"))?;
        Ok(InteractionEmission {
            show: &self.show,
            interaction,
        })
    }
}
