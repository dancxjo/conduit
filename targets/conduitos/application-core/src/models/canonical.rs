//! Canonical publication custody over the actual selected retained actor.
use super::*;
use conduit_body::BodyId;
use conduit_face::{
    Face, FaceContributionBasis, FaceFragment, FaceInteraction, ManifestationLifecycle, MaskShow,
};
use patchbay_application::{
    PatchbayApplicationRefusal, PatchbayCommandRefusal, PatchbayResidentCommand,
};

pub(super) struct Publication {
    body: BodyId,
    basis: FaceContributionBasis,
    face: Face,
    show: MaskShow,
}
impl Models {
    fn patchbay(&mut self, slot: u8) -> Result<&mut Patchbay, Refusal> {
        match self
            .applications
            .get_mut(usize::from(slot))
            .and_then(Option::as_mut)
        {
            Some(Application::Patchbay(p)) => Ok(p),
            _ => Err(Refusal::Order),
        }
    }
    pub fn patchbay_fragment(
        &self,
        slot: u8,
        body: BodyId,
        basis: FaceContributionBasis,
        actions_admitted: bool,
    ) -> Result<Reply, Refusal> {
        let Some(Application::Patchbay(p)) = self
            .applications
            .get(usize::from(slot))
            .and_then(Option::as_ref)
        else {
            return Err(Refusal::Order);
        };
        let port = p.ports[p.selected].as_ref().ok_or(Refusal::Order)?;
        let revision = port.revision();
        match port.face_fragment(basis, &body, actions_admitted) {
            Ok(fragment) => Ok(Reply::PatchbayFragment {
                slot,
                target: p.selected as u8,
                revision,
                fragment,
            }),
            Err(_) => Ok(refused(
                slot,
                p.selected,
                revision,
                CanonicalRefusal::Projection,
            )),
        }
    }
    pub fn patchbay_publish(
        &mut self,
        slot: u8,
        body: BodyId,
        basis: FaceContributionBasis,
        face: Face,
        show: MaskShow,
    ) -> Result<Reply, Refusal> {
        let p = self.patchbay(slot)?;
        let port = p.ports[p.selected].as_ref().ok_or(Refusal::Order)?;
        let revision = port.revision();
        // A rejected replacement revokes prior evidence; it cannot reopen or
        // replenish a consumed Show or silently retain old presentation authority.
        p.publication = None;
        let fail = |refusal| Ok(refused(slot, p.selected, revision, refusal));
        if face.validate().is_err() || show.validate(&face).is_err() {
            return fail(CanonicalRefusal::InvalidFace);
        }
        if show.show.lifecycle != ManifestationLifecycle::Available {
            return fail(CanonicalRefusal::UnacknowledgedShow);
        }
        if show.show.play_sequence <= p.publication_sequence {
            return fail(CanonicalRefusal::ShowConsumed);
        }
        if face.basis.body_id.as_ref() != Some(&body)
            || !patchbay_application::patchbay_face_contribution_matches(&face, &basis)
            || basis
                .required_interaction_context
                .as_ref()
                .is_some_and(|required| *required != face.interaction_context.identity)
        {
            return fail(CanonicalRefusal::WrongOwner);
        }
        let matches = [true, false].into_iter().any(|actions| {
            port.face_fragment(basis.clone(), &body, actions)
                .is_ok_and(|fragment| contains_fragment(&face, &fragment))
        });
        if !matches {
            return fail(CanonicalRefusal::InvalidContribution);
        }
        p.canonical = true;
        p.publication_sequence = show.show.play_sequence;
        p.publication = Some(Box::new(Publication {
            body,
            basis,
            face,
            show,
        }));
        Ok(Reply::PatchbayPublished {
            slot,
            target: p.selected as u8,
            revision,
        })
    }
    pub fn patchbay_consume(&mut self, slot: u8, input: &[u8]) -> Result<Reply, Refusal> {
        self.patchbay_consume_preflight(slot, input, crate::data::preflight_canonical_fixture)
    }
    pub(crate) fn patchbay_consume_preflight(
        &mut self,
        slot: u8,
        input: &[u8],
        mut preflight: impl FnMut(&Reply) -> Result<(), Refusal>,
    ) -> Result<Reply, Refusal> {
        let p = self.patchbay(slot)?;
        let port = p.ports[p.selected].as_mut().ok_or(Refusal::Order)?;
        let revision = port.revision();
        let target = p.selected as u8;
        // Remove before decoding or semantic admission: even a malformed or
        // refused occurrence consumes this acknowledged Show exactly once.
        let Some(publication) = p.publication.take() else {
            return Ok(refused(
                slot,
                p.selected,
                revision,
                CanonicalRefusal::NoPublication,
            ));
        };
        let failed = |refusal| Reply::PatchbayConsumed {
            slot,
            target,
            revision,
            proposal: None,
            refusal: Some(refusal),
        };
        let interaction = match FaceInteraction::decode(input) {
            Ok(interaction) => interaction,
            Err(error) => return Ok(failed(CanonicalRefusal::InvalidInteraction(error))),
        };
        let prepared = match port.admit_face_command(
            &publication.body,
            publication.basis.clone(),
            &publication.face,
            &publication.show,
            &interaction,
        ) {
            Ok(prepared) => prepared,
            Err(error) => return Ok(failed(map_refusal(error))),
        };
        // Preflight the exact potentially returned proposal before mutating the
        // actual actor. Maximum revision plus fixed slack also admits all typed
        // semantic refusal variants. No legacy view/canvas is generated here.
        let proposal = match prepared.command() {
            PatchbayResidentCommand::EditSubject {
                expanded_plot_id,
                subject_identity,
            } => Some(Proposal::EditCurrent(
                patchbay_application::PatchbayApplicationRequest::EditCurrent {
                    expanded_plot_id: expanded_plot_id.clone(),
                    subject_identity: subject_identity.clone(),
                },
            )),
            PatchbayResidentCommand::ChangeMasks { body_plan_id, mode } => {
                Some(Proposal::EditCurrent(
                    patchbay_application::PatchbayApplicationRequest::ChangeMasks {
                        body_plan_id: body_plan_id.clone(),
                        mode: *mode,
                    },
                ))
            }
            PatchbayResidentCommand::SelectPlot { .. } => None,
        };
        let witness = Reply::PatchbayConsumed {
            slot,
            target,
            revision: u32::MAX,
            proposal,
            refusal: None,
        };
        if preflight(&witness).is_err() {
            return Ok(failed(CanonicalRefusal::Capacity));
        }
        match port.apply_face_interaction(
            input,
            &publication.body,
            &publication.basis,
            &publication.face,
            &publication.show,
        ) {
            Ok(proposal) => Ok(Reply::PatchbayConsumed {
                slot,
                target,
                revision: port.revision(),
                proposal: proposal.map(Proposal::EditCurrent),
                refusal: None,
            }),
            Err(error) => Ok(failed(map_refusal(error))),
        }
    }
}
fn refused(slot: u8, target: usize, revision: u32, refusal: CanonicalRefusal) -> Reply {
    Reply::PatchbayRefused {
        slot,
        target: target as u8,
        revision,
        refusal,
    }
}
fn contains_fragment(face: &Face, fragment: &FaceFragment) -> bool {
    macro_rules! included {
        ($field:ident) => {
            fragment
                .$field
                .iter()
                .all(|item| face.$field.contains(item))
        };
    }
    included!(subjects)
        && included!(relationships)
        && included!(composition)
        && included!(properties)
        && included!(text)
        && included!(actions)
        && included!(disclosures)
        && included!(temporal_references)
        && included!(temporal_facts)
        && face
            .actions
            .iter()
            .filter(|action| {
                fragment
                    .subjects
                    .iter()
                    .any(|subject| subject.identity == action.target)
            })
            .all(|action| fragment.actions.contains(action))
}
fn map_refusal(error: PatchbayCommandRefusal) -> CanonicalRefusal {
    use PatchbayCommandRefusal::*;
    match error {
        WrongOwner => CanonicalRefusal::WrongOwner,
        StaleState => CanonicalRefusal::StaleState,
        InvalidContribution => CanonicalRefusal::InvalidContribution,
        InvalidInteraction(error) => CanonicalRefusal::InvalidInteraction(error),
        InvalidActionContract => CanonicalRefusal::InvalidActionContract,
        UnknownAction => CanonicalRefusal::UnknownAction,
        UnknownSubject => CanonicalRefusal::UnknownSubject,
        Projection(_) => CanonicalRefusal::Projection,
        Domain(error) => CanonicalRefusal::Domain(match error {
            PatchbayApplicationRefusal::Graph(_) => CanonicalDomainRefusal::Graph,
            PatchbayApplicationRefusal::Event(_) => CanonicalDomainRefusal::Event,
            PatchbayApplicationRefusal::UnknownAction => CanonicalDomainRefusal::UnknownAction,
            PatchbayApplicationRefusal::NoSubject => CanonicalDomainRefusal::NoSubject,
            PatchbayApplicationRefusal::RevisionExhausted => {
                CanonicalDomainRefusal::RevisionExhausted
            }
            PatchbayApplicationRefusal::EmptyActivePlots => {
                CanonicalDomainRefusal::EmptyActivePlots
            }
            PatchbayApplicationRefusal::TooManyActivePlots => {
                CanonicalDomainRefusal::TooManyActivePlots
            }
        }),
    }
}

#[cfg(test)]
#[path = "canonical_tests.rs"]
mod tests;
