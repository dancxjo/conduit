//! Face/Show correlation only. This does not admit physical input availability.
use crate::{
    Face, FaceAction, ManifestationLifecycle, MaskShow, SourceProducedInputBindingProposal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputBindingCorrelationRefusal {
    Face,
    Show,
    StaleFace,
    StaleShow,
    Action,
}

pub struct FaceCorrelatedInputBindingProposal<'a> {
    produced: &'a SourceProducedInputBindingProposal,
    action: &'a FaceAction,
}
impl FaceCorrelatedInputBindingProposal<'_> {
    pub fn produced(&self) -> &SourceProducedInputBindingProposal {
        self.produced
    }
    pub fn action(&self) -> &FaceAction {
        self.action
    }
}

/// The caller supplies authoritative current Face/Show snapshots. The result
/// retains their exact action, but has no controller or remapping authority.
pub fn correlate_input_binding_proposal<'a>(
    produced: &'a SourceProducedInputBindingProposal,
    face: &'a Face,
    show: &MaskShow,
) -> Result<FaceCorrelatedInputBindingProposal<'a>, InputBindingCorrelationRefusal> {
    face.validate()
        .map_err(|_| InputBindingCorrelationRefusal::Face)?;
    show.validate(face)
        .map_err(|_| InputBindingCorrelationRefusal::Show)?;
    if show.show.lifecycle != ManifestationLifecycle::Available {
        return Err(InputBindingCorrelationRefusal::Show);
    }
    let proposal = produced.proposal();
    if proposal.face_identity() != face.identity.as_str()
        || *proposal.offered_action().source_face_revision() != face.revision
    {
        return Err(InputBindingCorrelationRefusal::StaleFace);
    }
    if proposal.show_identity() != show.show_id.as_str() {
        return Err(InputBindingCorrelationRefusal::StaleShow);
    }
    let action = face
        .resolve_action(face.revision, proposal.offered_action().action_identity())
        .map_err(|_| InputBindingCorrelationRefusal::Action)?;
    Ok(FaceCorrelatedInputBindingProposal { produced, action })
}
