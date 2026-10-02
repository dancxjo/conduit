//! The same biography-driven resident Tutorial used by browser Workspace.
use super::{NativeApplicationRequest, WorksetRefusal, play::PlayRefusal};
use alloc::vec::Vec;
use conduit_body::BodyBiographyEvidence;
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_presentation::{ApplicationEvent, ApplicationView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TutorialAction {
    Wake,
    InspectLifecycle,
    OpenLibrary,
    InviteHost,
    UseCurrent,
}

pub(super) fn selected_application(placement: &PlannedGear) -> Result<&str, WorksetRefusal> {
    let mut entries = placement
        .configuration
        .iter()
        .filter(|entry| entry.key == "application");
    let Some(entry) = entries.next() else {
        return Err(WorksetRefusal::Plan);
    };
    if entries.next().is_some() {
        return Err(WorksetRefusal::Plan);
    }
    match &entry.value {
        ConfigurationValue::Text(value)
            if matches!(value.as_str(), "tutorial" | "tour" | "patchbay") =>
        {
            Ok(value)
        }
        _ => Err(WorksetRefusal::Plan),
    }
}

pub(super) struct TutorialApplication {
    view: ApplicationView,
}
impl TutorialApplication {
    pub(super) fn prepare(
        evidence: &BodyBiographyEvidence,
        playback: conduit_tutorial_plot::TutorialPlayback,
    ) -> Result<Self, WorksetRefusal> {
        evidence.validate().map_err(|_| WorksetRefusal::Plan)?;
        // The finite view revision counts retained records, not the opaque birth nonce.
        let revision = u32::try_from(evidence.records.len()).map_err(|_| WorksetRefusal::Plan)?;
        let view = conduit_tutorial_plot::presentation_from_evidence(evidence, revision, playback)
            .and_then(|view| view.lower())
            .map_err(|_| WorksetRefusal::Plan)?;
        if view.encode().map_err(|_| WorksetRefusal::Plan)?.len()
            > super::application_delivery::VIEW_BYTES as usize
        {
            return Err(WorksetRefusal::Resource);
        }
        Ok(Self { view })
    }
    pub(super) fn apply(
        &self,
        input: &[u8],
    ) -> Result<(Vec<u8>, Option<NativeApplicationRequest>), PlayRefusal> {
        let action = if input.is_empty() {
            None
        } else {
            let event =
                ApplicationEvent::decode(input, &self.view).map_err(|_| PlayRefusal::Kernel)?;
            let action = match event.action.as_str() {
                "body.wake" => TutorialAction::Wake,
                "body.inspect-lifecycle" => TutorialAction::InspectLifecycle,
                "body.open-library" => TutorialAction::OpenLibrary,
                "body.invite-host" => TutorialAction::InviteHost,
                "body.use-current" => TutorialAction::UseCurrent,
                _ => return Err(PlayRefusal::Kernel),
            };
            Some(NativeApplicationRequest::Tutorial(action))
        };
        Ok((self.view.encode().map_err(|_| PlayRefusal::Kernel)?, action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_application_is_exact_and_never_defaults_to_legacy_tour() {
        let (ids, offer) = crate::native_workset::tests::fixture();
        let wake = crate::native_workset::tests::wake(&[crate::native_workset::NativePlot::Tour]);
        let prepared = crate::native_workset::prepare(&wake, &ids, &offer, "build").unwrap();
        assert!(matches!(
            crate::native_workset::NativeWorksetPlay::prepare(&prepared),
            Err(WorksetRefusal::Plan)
        ));
        let mut placement = prepared.plan.plots[0].plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == "application/retained")
            .unwrap()
            .clone();
        assert_eq!(selected_application(&placement), Ok("tutorial"));
        let entry = placement
            .configuration
            .iter_mut()
            .find(|e| e.key == "application")
            .unwrap();
        entry.value = ConfigurationValue::Text("tour".into());
        assert_eq!(selected_application(&placement), Ok("tour"));
        placement
            .configuration
            .iter_mut()
            .find(|e| e.key == "application")
            .unwrap()
            .value = ConfigurationValue::Text("unknown".into());
        assert_eq!(selected_application(&placement), Err(WorksetRefusal::Plan));
        placement.configuration.retain(|e| e.key != "application");
        assert_eq!(selected_application(&placement), Err(WorksetRefusal::Plan));
    }
}
