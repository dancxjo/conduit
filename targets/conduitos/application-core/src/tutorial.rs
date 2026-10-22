//! The canonical biography-driven resident Tutorial, shared with local Hosts.
use crate::{
    VIEW_BYTES,
    data::{Proposal, Refusal, TutorialAction},
};
use alloc::vec::Vec;
use conduit_body::BodyBiographyEvidence;
use conduit_face::{ApplicationEvent, ApplicationView};
pub struct Tutorial {
    view: ApplicationView,
}
impl Tutorial {
    pub fn prepare(evidence: &BodyBiographyEvidence) -> Result<Self, Refusal> {
        let view = project(evidence, false)?;
        Ok(Self { view })
    }
    pub fn apply(&self, input: &[u8]) -> Result<(Vec<u8>, Option<Proposal>), Refusal> {
        let action = if input.is_empty() {
            None
        } else {
            let event =
                ApplicationEvent::decode(input, &self.view).map_err(|_| Refusal::Application)?;
            Some(Proposal::Tutorial(match event.action.as_str() {
                "body.wake" => TutorialAction::Wake,
                "body.inspect-lifecycle" => TutorialAction::InspectLifecycle,
                "body.open-library" => TutorialAction::OpenLibrary,
                "body.invite-host" => TutorialAction::InviteHost,
                "body.use-current" => TutorialAction::UseCurrent,
                _ => return Err(Refusal::Application),
            }))
        };
        Ok((
            self.view.encode().map_err(|_| Refusal::Application)?,
            action,
        ))
    }
}

/// Shared canonical projection. A stateless read does not touch retained models
/// or issue any external-effect proposal, including before model initialization.
pub fn project(evidence: &BodyBiographyEvidence, lulled: bool) -> Result<ApplicationView, Refusal> {
    evidence.validate().map_err(|_| Refusal::Application)?;
    let revision = u32::try_from(evidence.records.len()).map_err(|_| Refusal::Bounds)?;
    let view = conduit_tutorial_plot::face_from_evidence(
        evidence,
        revision,
        if lulled {
            conduit_tutorial_plot::TutorialPlayback::Lulled
        } else {
            conduit_tutorial_plot::TutorialPlayback::Playing
        },
    )
    .and_then(|view| view.lower())
    .map_err(|_| Refusal::Application)?;
    if view.encode().map_err(|_| Refusal::Application)?.len() > VIEW_BYTES {
        return Err(Refusal::Bounds);
    }
    Ok(view)
}
