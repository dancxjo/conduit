//! Exact body membership and foreground view, supplied by ProductJourney.
use super::{Error, FrontDoor};
use crate::product_journey::{JourneyProjection, WorkspaceProjection};
use alloc::{format, string::String};

pub(super) enum WorkspaceRefusal {
    Startup(String),
    Play(String),
}
impl WorkspaceRefusal {
    pub(super) fn reason(&self) -> &str {
        match self {
            Self::Startup(reason) | Self::Play(reason) => reason,
        }
    }
    pub(super) fn key(&self) -> &'static str {
        match self {
            Self::Startup(_) => "startup-refusal",
            Self::Play(_) => "play-refusal",
        }
    }
    #[allow(dead_code)]
    pub(super) fn heading(&self) -> &'static str {
        match self {
            Self::Startup(_) => "Wake could not finish. Details:",
            Self::Play(_) => "Play stopped. Details:",
        }
    }
}

impl FrontDoor {
    /// Refresh from the authoritative lifecycle and its exact resident workset.
    pub fn observe_product(
        &mut self,
        journey: &crate::product_journey::ProductJourney,
    ) -> Result<(), Error> {
        if let Some(workspace) = journey.workspace_projection() {
            let application_view = journey.foreground_application_view().cloned();
            if let Some(view) = &application_view {
                view.validate().map_err(|_| Error::Presentation)?;
            }
            self.observe_body(journey.projection(), workspace)?;
            self.application_view = application_view;
            Ok(())
        } else {
            self.application_view = None;
            self.observe_journey(journey.projection())
        }
    }

    pub fn play_refused(&mut self, reason: &str) -> Result<(), Error> {
        let status = self.journey.as_ref().map(|journey| journey.status);
        if !matches!(
            status,
            Some(
                crate::product_journey::JourneyStatus::Stopped
                    | crate::product_journey::JourneyStatus::InputUnavailable
            )
        ) {
            return Err(Error::Presentation);
        }
        self.refusal = Some(WorkspaceRefusal::Play(reason.into()));
        self.advance()
    }

    pub fn observe_body(
        &mut self,
        journey: JourneyProjection,
        workspace: WorkspaceProjection,
    ) -> Result<(), Error> {
        if journey.body_id.as_ref() != Some(&workspace.body_id)
            || journey.revision != workspace.revision
            || workspace.plots.is_empty()
            || workspace.plots.len() > crate::native_workset::NATIVE_PLOT_CAPACITY
            || workspace
                .plots
                .iter()
                .filter(|plot| plot.foreground)
                .count()
                != 1
        {
            return Err(Error::Presentation);
        }
        if self
            .journey
            .as_ref()
            .is_some_and(|previous| previous.revision > journey.revision)
            || workspace.plots.iter().enumerate().any(|(index, plot)| {
                workspace.plots[..index]
                    .iter()
                    .any(|prior| prior.plot == plot.plot)
            })
        {
            return Err(Error::Presentation);
        }
        let selected = workspace
            .plots
            .iter()
            .find(|plot| plot.foreground)
            .ok_or(Error::Presentation)?;
        if journey.source_document_id.as_ref() != Some(&selected.plot.source_document_id)
            || journey.checked_plot_id.as_ref() != Some(&selected.plot.checked_plot_id)
            || self
                .journey
                .as_ref()
                .and_then(|previous| previous.body_id.as_ref())
                .is_some_and(|body| body != &workspace.body_id)
            || journey.host_id != self.host_id
            || journey.boot_id != self.boot_id
            || journey.offer_generation != self.offer_generation
        {
            return Err(Error::Presentation);
        }
        if let Some(previous) = &self.workspace {
            let unchanged = previous.plots.len() == workspace.plots.len()
                && previous
                    .plots
                    .iter()
                    .zip(&workspace.plots)
                    .all(|(left, right)| left.plot == right.plot && left.title == right.title);
            let admitted_append = workspace.plots.len() == previous.plots.len() + 1
                && previous
                    .plots
                    .iter()
                    .zip(&workspace.plots)
                    .all(|(left, right)| left.plot == right.plot && left.title == right.title)
                && self.journey.as_ref().is_some_and(|prior| {
                    prior
                        .workload_revision
                        .zip(journey.workload_revision)
                        .is_some_and(|(left, right)| left.checked_add(1) == Some(right))
                })
                && journey.workload_sign_id.is_some();
            // Selection and output cannot rewrite membership. The sole growth
            // case is an exact append backed by the Body/Wake workload event.
            if !unchanged && !admitted_append {
                return Err(Error::Presentation);
            }
            if admitted_append {
                let appended = workspace.plots.last().ok_or(Error::Presentation)?;
                let kind = crate::native_workset::resolve(&appended.plot)
                    .map_err(|_| Error::Presentation)?;
                if kind.title() != appended.title {
                    return Err(Error::Presentation);
                }
            }
        } else {
            for plot in &workspace.plots {
                let kind =
                    crate::native_workset::resolve(&plot.plot).map_err(|_| Error::Presentation)?;
                if kind.title() != plot.title {
                    return Err(Error::Presentation);
                }
            }
        }
        self.revision.checked_add(1).ok_or(Error::Presentation)?;
        self.source_document_id = journey
            .source_document_id
            .clone()
            .ok_or(Error::Presentation)?;
        self.checked_plot_id = journey.checked_plot_id.clone().ok_or(Error::Presentation)?;
        self.plot_subject = format!("plot/{}", self.checked_plot_id.as_str());
        if self.selected_subject.starts_with("plot/") {
            self.selected_subject = self.plot_subject.clone();
        }
        self.plot_open = false;
        // A newly admitted play supersedes an earlier startup/input refusal.
        if journey.status == crate::product_journey::JourneyStatus::QuiescentAwaitingInput {
            self.refusal = None;
        }
        self.journey = Some(journey);
        self.workspace = Some(workspace);
        self.advance()
    }
}

#[cfg(test)]
mod tests;
