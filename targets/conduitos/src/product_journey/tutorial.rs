//! Native operation adapter for shared Workspace Tutorial actions.
use super::*;
use crate::native_workset::TutorialAction;
use conduit_presentation::{ApplicationEvent, ApplicationView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TutorialRefusal {
    InvalidAction,
    LibraryUnavailable,
    InvitationUnavailable,
    LifecycleInspectionUnavailable,
    NoWorkingPlot,
    Lifecycle(JourneyError),
}
impl TutorialRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidAction => "tutorial-action-stale-or-invalid",
            Self::LibraryUnavailable => {
                "tutorial-library-unavailable: native catalog editing is not implemented"
            }
            Self::InvitationUnavailable => {
                "tutorial-invitation-unavailable: native Host invitation is not implemented"
            }
            Self::LifecycleInspectionUnavailable => {
                "tutorial-lifecycle-inspection-unavailable: native Body biography reader is not implemented"
            }
            Self::NoWorkingPlot => {
                "tutorial-working-plot-unavailable: this Body has no installed working Plot"
            }
            Self::Lifecycle(error) => error.as_str(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TutorialSurface {
    Current,
    Lifecycle,
}

impl ProductJourney {
    pub fn foreground_is_tutorial(&self) -> bool {
        self.plots[self.foreground_index()] == Some(NativePlot::Tour)
    }

    pub fn tutorial_view(&self) -> Result<ApplicationView, TutorialRefusal> {
        let evidence = self.biography().ok_or(TutorialRefusal::InvalidAction)?;
        let playback = if matches!(
            self.status,
            JourneyStatus::BornLulled | JourneyStatus::Lulled
        ) {
            conduit_tutorial_plot::TutorialPlayback::Lulled
        } else {
            conduit_tutorial_plot::TutorialPlayback::Playing
        };
        conduit_tutorial_plot::presentation_from_evidence(
            evidence,
            u32::try_from(evidence.records.len()).map_err(|_| TutorialRefusal::InvalidAction)?,
            playback,
        )
        .and_then(|view| view.lower())
        .map_err(|_| TutorialRefusal::InvalidAction)
    }

    pub fn accept_tutorial_action(
        &mut self,
        event: &ApplicationEvent,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
    ) -> Result<TutorialSurface, TutorialRefusal> {
        let view = self.tutorial_view()?;
        event
            .encode(&view)
            .map_err(|_| TutorialRefusal::InvalidAction)?;
        let action = match event.action.as_str() {
            "body.wake" => TutorialAction::Wake,
            "body.inspect-lifecycle" => TutorialAction::InspectLifecycle,
            "body.open-library" => TutorialAction::OpenLibrary,
            "body.invite-host" => TutorialAction::InviteHost,
            "body.use-current" => TutorialAction::UseCurrent,
            _ => return Err(TutorialRefusal::InvalidAction),
        };
        self.apply_tutorial_action(action, identities, offer, build_id)
    }

    pub fn apply_tutorial_action(
        &mut self,
        action: TutorialAction,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
    ) -> Result<TutorialSurface, TutorialRefusal> {
        match action {
            TutorialAction::OpenLibrary => Err(TutorialRefusal::LibraryUnavailable),
            TutorialAction::InviteHost => Err(TutorialRefusal::InvitationUnavailable),
            TutorialAction::InspectLifecycle => {
                Err(TutorialRefusal::LifecycleInspectionUnavailable)
            }
            TutorialAction::UseCurrent => {
                let body = self.body().ok_or(TutorialRefusal::InvalidAction)?;
                let target = self
                    .last_working_plot
                    .as_ref()
                    .filter(|plot| body.workset.contains(plot))
                    .cloned()
                    .or_else(|| {
                        body.workset
                            .plots()
                            .iter()
                            .find(|plot| {
                                matches!(
                                    native_workset::resolve(plot),
                                    Ok(NativePlot::KeyboardCanvas | NativePlot::MemoryLantern)
                                )
                            })
                            .cloned()
                    })
                    .ok_or(TutorialRefusal::NoWorkingPlot)?;
                self.select_plot(&target, self.revision)
                    .map_err(TutorialRefusal::Lifecycle)?;
                Ok(TutorialSurface::Current)
            }
            TutorialAction::Wake => {
                self.wake(identities, offer, build_id)
                    .map_err(TutorialRefusal::Lifecycle)?;
                self.advance().map_err(TutorialRefusal::Lifecycle)?;
                self.plan(identities, offer, build_id)
                    .map_err(TutorialRefusal::Lifecycle)?;
                self.advance().map_err(TutorialRefusal::Lifecycle)?;
                self.play().map_err(TutorialRefusal::Lifecycle)?;
                self.advance().map_err(TutorialRefusal::Lifecycle)?;
                Ok(TutorialSurface::Current)
            }
        }
    }
}
