//! Host-owned zero-Body encounter over the existing Birth draft.
//!
//! No Body, Plan, Play or Mask Show is invented here. A Host adapter must own
//! delivery and authorization. Only the ordinary explicit Birth action returns
//! a selection for that Host's existing lifecycle boundary.
use crate::{BirthActionOutcome, BirthActions, BirthDraft, BirthDraftRefusal};
use alloc::string::String;
use conduit_core::{BootId, HostId, PlanId, PlotIdentity};
use conduit_presentation::{FaceInteraction, FaceInteractionRefusal, MaskShow, Presentation};
mod projection;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirthFaceBasis {
    pub host_id: HostId,
    pub boot_id: BootId,
    /// Host-owned identity distinguishing separate drafts on the same Boot.
    pub encounter_id: String,
    /// Actual Host-owned producer identities; never an invented Body or Plan.
    pub producer_plot: PlotIdentity,
    pub producer_plan_id: PlanId,
}

/// A Birth encounter presented directly by its Host, before any producer
/// Plot or Plan exists. All producer identities are absent together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOwnedBirthFaceBasis {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub encounter_id: String,
}

pub(super) enum FaceBasis<'a> {
    Planned(&'a BirthFaceBasis),
    HostOwned(&'a HostOwnedBirthFaceBasis),
}

impl FaceBasis<'_> {
    fn host_id(&self) -> &HostId {
        match self {
            Self::Planned(basis) => &basis.host_id,
            Self::HostOwned(basis) => &basis.host_id,
        }
    }
    fn boot_id(&self) -> &BootId {
        match self {
            Self::Planned(basis) => &basis.boot_id,
            Self::HostOwned(basis) => &basis.boot_id,
        }
    }
    fn encounter_id(&self) -> &str {
        match self {
            Self::Planned(basis) => &basis.encounter_id,
            Self::HostOwned(basis) => &basis.encounter_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BirthFaceRefusal {
    InvalidProjection,
    Presentation(conduit_presentation::PresentationError),
    View(conduit_presentation::SemanticPresentationRefusal),
    UnsupportedMechanism,
    Interaction(FaceInteractionRefusal),
    WrongEvent,
    Draft(BirthDraftRefusal),
}

impl BirthDraft {
    pub fn face(&self, basis: &BirthFaceBasis) -> Result<Presentation, BirthFaceRefusal> {
        projection::project(self, FaceBasis::Planned(basis)).map(|(face, _)| face)
    }

    pub fn host_owned_face(
        &self,
        basis: &HostOwnedBirthFaceBasis,
    ) -> Result<Presentation, BirthFaceRefusal> {
        projection::project(self, FaceBasis::HostOwned(basis)).map(|(face, _)| face)
    }

    /// Validate against the current draft, never against a caller-supplied Face.
    /// The result remains a selection; this method cannot create a Body.
    pub fn apply_face_interaction(
        &mut self,
        basis: &BirthFaceBasis,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<BirthActionOutcome, BirthFaceRefusal> {
        self.apply_projected_interaction(FaceBasis::Planned(basis), show, input)
    }

    pub fn apply_host_owned_face_interaction(
        &mut self,
        basis: &HostOwnedBirthFaceBasis,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<BirthActionOutcome, BirthFaceRefusal> {
        self.apply_projected_interaction(FaceBasis::HostOwned(basis), show, input)
    }

    fn apply_projected_interaction(
        &mut self,
        basis: FaceBasis<'_>,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<BirthActionOutcome, BirthFaceRefusal> {
        let (face, events) = projection::project(self, basis)?;
        input
            .validate_against(&face, show)
            .map_err(BirthFaceRefusal::Interaction)?;
        let event = events
            .iter()
            .find(|(id, _)| id == &input.action_id)
            .map(|(_, event)| *event)
            .ok_or(BirthFaceRefusal::WrongEvent)?;
        let value = match input.arguments.first() {
            None => "",
            Some(argument) if argument.value_kind == "value/bool" => {
                if argument.value == [1] {
                    "true"
                } else {
                    "false"
                }
            }
            Some(argument) => core::str::from_utf8(&argument.value).map_err(|_| {
                BirthFaceRefusal::Interaction(FaceInteractionRefusal::MalformedEncoding)
            })?,
        };
        self.apply_event(self.revision(), &input.action_id, event, value)
            .map_err(BirthFaceRefusal::Draft)
    }
}
