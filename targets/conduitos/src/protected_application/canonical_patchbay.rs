//! Root custody of actual acknowledged evidence, never another Patchbay actor.
use super::*;
use conduit_body::BodyId;
use conduit_face::{Face, FaceContributionBasis, FaceFragment, FaceInteraction, MaskShow};

pub(super) struct Publication {
    body: BodyId,
    basis: FaceContributionBasis,
    face: Face,
    show: MaskShow,
    target: u8,
    revision: u32,
}

impl Owner {
    /// The caller binds this query to its exact resident Plan/Play and admitted
    /// return route. A projected Available action is not an installed route.
    pub fn patchbay_fragment(
        &self,
        slot: usize,
        body: BodyId,
        basis: FaceContributionBasis,
        actions_admitted: bool,
    ) -> Result<Reply, Refusal> {
        self.canonical_slot(slot)?;
        let reply = self.read_command(&Command::PatchbayFragment {
            slot: slot as u8,
            body,
            basis: basis.clone(),
            actions_admitted,
        })?;
        let result = (|| match &reply {
            Reply::PatchbayFragment {
                slot: returned,
                target,
                revision,
                fragment,
            } => {
                self.canonical_identity(slot, *returned, *target)?;
                self.check_fragment(slot, *revision, fragment, &basis)
            }
            Reply::PatchbayRefused {
                slot: returned,
                target,
                ..
            } => self.canonical_identity(slot, *returned, *target),
            _ => Err(Refusal::Malformed),
        })();
        self.read_result(result)?;
        Ok(reply)
    }

    /// Supply only evidence independently retained from the actual Mask receipt.
    /// Submitted interaction bytes cannot replace any part of this publication.
    pub fn patchbay_publish(
        &mut self,
        slot: usize,
        body: BodyId,
        basis: FaceContributionBasis,
        face: Face,
        show: MaskShow,
    ) -> Result<Reply, Refusal> {
        self.canonical_slot(slot)?;
        // Replacement starts by retiring the prior receipt, including rejection.
        self.publications[slot] = None;
        if face.basis.body_id.as_ref() != Some(&body)
            || face.validate().is_err()
            || show.validate(&face).is_err()
            || show.show.lifecycle != conduit_face::ManifestationLifecycle::Available
            || self.publication_sequences[slot].is_some_and(|last| show.show.play_sequence <= last)
            || !patchbay_application::patchbay_face_contribution_matches(&face, &basis)
        {
            return self.guard(Err(Refusal::Order));
        }
        let checked_revision = revision(&face.properties, &basis).ok_or(Refusal::Malformed);
        let revision = self.guard(checked_revision)?;
        // Retire the prior Root receipt before crossing a fallible transport.
        // Uncertainty cannot revive it or authorize replay of a prior action.
        self.publications[slot] = None;
        let reply = self.command(&Command::PatchbayPublish {
            slot: slot as u8,
            body: body.clone(),
            basis: basis.clone(),
            face: face.clone(),
            show: show.clone(),
        })?;
        let result = (|| match &reply {
            Reply::PatchbayPublished {
                slot: returned,
                target,
                revision: observed,
            } => {
                self.canonical_identity(slot, *returned, *target)?;
                if *observed != revision {
                    return Err(Refusal::Malformed);
                }
                self.publication_sequences[slot] = Some(show.show.play_sequence);
                self.canonical_slots[slot] = true;
                self.publications[slot] = Some(Box::new(Publication {
                    body,
                    basis,
                    face,
                    show,
                    target: *target,
                    revision,
                }));
                Ok(())
            }
            Reply::PatchbayRefused {
                slot: returned,
                target,
                ..
            } => self.canonical_identity(slot, *returned, *target),
            _ => Err(Refusal::Malformed),
        })();
        self.guard(result)?;
        Ok(reply)
    }

    /// Called with bytes from the exact resident ingress Host Call. Root checks
    /// the returned request against genuine input before granting external work.
    pub fn patchbay_consume(&mut self, slot: usize, input: &[u8]) -> Result<Reply, Refusal> {
        self.canonical_slot(slot)?;
        let publication = self.publications[slot].take().ok_or(Refusal::Order)?;
        if input.len() > conduit_face::MAX_FACE_INTERACTION_BYTES {
            return self.guard(Err(Refusal::Bounds));
        }
        if publication.target != self.selected[slot]
            || publication.face.basis.body_id.as_ref() != Some(&publication.body)
            || !patchbay_application::patchbay_face_contribution_matches(
                &publication.face,
                &publication.basis,
            )
        {
            return self.guard(Err(Refusal::Malformed));
        }
        let interaction = FaceInteraction::decode(input).and_then(|interaction| {
            interaction.validate_against(&publication.face, &publication.show)?;
            Ok(interaction)
        });
        let reply = self.command(&Command::PatchbayConsume {
            slot: slot as u8,
            input: input.into(),
        })?;
        let result = (|| match &reply {
            Reply::PatchbayConsumed {
                slot: returned,
                target,
                revision,
                proposal,
                refusal,
            } => {
                self.canonical_identity(slot, *returned, *target)?;
                if refusal.is_some() {
                    if proposal.is_some() || *revision != publication.revision {
                        Err(Refusal::Malformed)
                    } else {
                        Ok(())
                    }
                } else {
                    let interaction = interaction.as_ref().map_err(|_| Refusal::Malformed)?;
                    if Some(*revision) != publication.revision.checked_add(1) {
                        return Err(Refusal::Malformed);
                    }
                    admit_proposal(
                        interaction,
                        self.graph(slot).ok_or(Refusal::Order)?,
                        self.topology.as_deref(),
                        proposal.as_ref(),
                    )
                }
            }
            // An acknowledged Root publication must not disappear privately.
            _ => Err(Refusal::Malformed),
        })();
        self.guard(result)?;
        Ok(reply)
    }

    fn canonical_slot(&self, slot: usize) -> Result<(), Refusal> {
        if !self.is_live() || self.kind(slot) != Some(Kind::Patchbay) {
            return Err(Refusal::Order);
        }
        Ok(())
    }
    fn canonical_identity(&self, slot: usize, returned: u8, target: u8) -> Result<(), Refusal> {
        if usize::from(returned) != slot || self.selected.get(slot).copied() != Some(target) {
            return Err(Refusal::Malformed);
        }
        Ok(())
    }
    fn check_fragment(
        &self,
        slot: usize,
        actor_revision: u32,
        fragment: &FaceFragment,
        basis: &FaceContributionBasis,
    ) -> Result<(), Refusal> {
        if &fragment.basis != basis
            || fragment.validate_bounds().is_err()
            || revision(&fragment.properties, basis) != Some(actor_revision)
        {
            return Err(Refusal::Malformed);
        }
        let graph = self.graph(slot).ok_or(Refusal::Order)?;
        let root = alloc::format!("patchbay/document/{}", basis.checked_plot_id.as_str());
        let plot = alloc::format!("plot/{}", graph.checked_plot_id.as_str());
        for action in &fragment.actions {
            let valid = if action.identity == patchbay_application::EDIT_CURRENT_ACTION_ID {
                action.target == plot
                    && action.intent == "patchbay/edit-subject@1"
                    && action.arguments.len() == 1
                    && action.arguments[0].name == "subject"
            } else if action.identity == patchbay_application::CHANGE_MASKS_ACTION_ID {
                action.target == root
                    && action.intent == "patchbay/change-masks@1"
                    && action.arguments.is_empty()
                    && self.topology.is_some()
            } else if action.identity
                == alloc::format!("{}0", patchbay_application::SELECT_PLOT_ACTION_PREFIX)
            {
                action.target == plot
                    && action.intent == "patchbay/select-plot@1"
                    && action.arguments.is_empty()
            } else {
                false
            };
            if !valid {
                return Err(Refusal::Malformed);
            }
        }
        Ok(())
    }
}

fn revision(
    properties: &[conduit_face::FaceProperty],
    basis: &FaceContributionBasis,
) -> Option<u32> {
    let root = alloc::format!("patchbay/document/{}", basis.checked_plot_id.as_str());
    let mut entries = properties
        .iter()
        .filter(|p| p.subject == root && p.name == "application-revision");
    let entry = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    let conduit_face::FacePropertyValue::Count(value) = entry.value else {
        return None;
    };
    u32::try_from(value).ok().filter(|revision| *revision != 0)
}

fn admit_proposal(
    input: &FaceInteraction,
    graph: &PatchbayGraph,
    topology: Option<&PatchbayMaskTopology>,
    proposal: Option<&Proposal>,
) -> Result<(), Refusal> {
    if input.action_id == patchbay_application::EDIT_CURRENT_ACTION_ID {
        let subject = input
            .arguments
            .iter()
            .find(|a| a.name == "subject")
            .and_then(|a| core::str::from_utf8(&a.value).ok())
            .ok_or(Refusal::Malformed)?;
        graph.subject_ref(subject).map_err(|_| Refusal::Malformed)?;
        match proposal {
            Some(Proposal::EditCurrent(
                patchbay_application::PatchbayApplicationRequest::EditCurrent {
                    expanded_plot_id,
                    subject_identity,
                },
            )) if expanded_plot_id == &graph.expanded_plot_id && subject_identity == subject => {
                Ok(())
            }
            _ => Err(Refusal::Malformed),
        }
    } else if input.action_id == patchbay_application::CHANGE_MASKS_ACTION_ID {
        let topology = topology.ok_or(Refusal::Malformed)?;
        match proposal {
            Some(Proposal::EditCurrent(
                patchbay_application::PatchbayApplicationRequest::ChangeMasks {
                    body_plan_id,
                    mode,
                },
            )) if body_plan_id == &topology.body_plan_id && *mode == topology.next_mode() => Ok(()),
            _ => Err(Refusal::Malformed),
        }
    } else if input.action_id
        == alloc::format!("{}0", patchbay_application::SELECT_PLOT_ACTION_PREFIX)
        && proposal.is_none()
    {
        Ok(())
    } else {
        Err(Refusal::Malformed)
    }
}
