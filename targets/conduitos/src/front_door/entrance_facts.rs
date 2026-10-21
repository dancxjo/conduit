//! Observational Root facts only; mode selection and text/layout policy live in Mask.
use super::{ConnectivityStatus, Error, FrontDoor};
use crate::product_bindings::binding_for_intent;
use conduitos_mask_core::{
    control,
    entrance::{BirthAction, Connectivity, Facts, Scope},
};
impl ConnectivityStatus {
    pub(super) const fn mask_status(self) -> conduitos_mask_core::entrance::ConnectivityStatus {
        use conduitos_mask_core::entrance::ConnectivityStatus as Status;
        match self {
            Self::Current => Status::Current,
            Self::PeerAttached => Status::PeerAttached,
            Self::ValueVisible => Status::ValueVisible,
            Self::Lost => Status::Lost,
        }
    }
}
impl FrontDoor {
    pub(super) fn entrance_facts(&self, width: u16, height: u16) -> Result<Facts, Error> {
        let face = self.face()?;
        face.validate().map_err(|_| Error::Face)?;
        let birth = face
            .actions
            .iter()
            .find(|a| a.intent == "conduit.intent/birth@1")
            .map(|action| {
                let unavailable = match &action.availability {
                    conduit_face::FaceActionAvailability::Available => None,
                    conduit_face::FaceActionAvailability::Unavailable { explanation, .. }
                    | conduit_face::FaceActionAvailability::Refused { explanation, .. } => {
                        Some(explanation.clone())
                    }
                };
                BirthAction {
                    binding: binding_for_intent(&action.intent)
                        .map_or("ENTER", |binding| binding.label)
                        .into(),
                    name: action.name.clone(),
                    unavailable,
                }
            });
        let journey = self.journey.as_ref();
        let facts = Facts {
            scope: Scope {
                control: control::Scope {
                    session: 0,
                    host: self.host_id.clone(),
                    boot: self.boot_id.clone(),
                },
                outer_revision: self.revision,
                journey_revision: self.journey.as_ref().map_or(0, |journey| journey.revision),
                width,
                height,
            },
            profile: self.profile_id.clone(),
            build: self.build_id.clone(),
            image: self.image_id.clone(),
            offer_count: self.offer_count,
            offer_generation: self.offer_generation.0,
            plot: self.plot_subject.clone(),
            source: self.source_document_id.as_str().into(),
            checked: self.checked_plot_id.as_str().into(),
            lifecycle: [
                journey
                    .and_then(|j| j.expanded_plot_id.as_ref())
                    .map(|id| id.as_str().into()),
                journey
                    .and_then(|j| j.body_id.as_ref())
                    .map(|id| id.as_str().into()),
                journey
                    .and_then(|j| j.wake_id.as_ref())
                    .map(|id| id.as_str().into()),
                journey
                    .and_then(|j| j.plan_id.as_ref())
                    .map(|id| id.as_str().into()),
                journey
                    .and_then(|j| j.active_play_id.as_ref())
                    .map(|id| id.as_str().into()),
                journey
                    .and_then(|j| j.input_sign_id.as_ref())
                    .map(|id| id.as_str().into()),
                journey
                    .and_then(|j| j.result_sign_id.as_ref())
                    .map(|id| id.as_str().into()),
            ],
            connectivity: self.connectivity.as_ref().map(|line| Connectivity {
                body: line.body_id.as_str().into(),
                line: line.line_id.clone(),
                status: line.status.mask_status(),
                value: line.value.clone(),
            }),
            birth,
        };
        facts.validate().map_err(|_| Error::Scene)?;
        Ok(facts)
    }
}
