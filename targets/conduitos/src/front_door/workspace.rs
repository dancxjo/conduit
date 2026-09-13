//! Exact Body membership and foreground view, supplied by ProductJourney.
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
            self.observe_body(journey.projection(), workspace)
        } else {
            self.observe_journey(journey.projection())
        }
    }

    pub fn play_refused(&mut self, reason: &str) -> Result<(), Error> {
        if self.journey.as_ref().map(|journey| journey.status)
            != Some(crate::product_journey::JourneyStatus::Stopped)
        {
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
            || workspace.forms.is_empty()
            || workspace.forms.len() > 2
            || workspace
                .forms
                .iter()
                .filter(|form| form.foreground)
                .count()
                != 1
        {
            return Err(Error::Presentation);
        }
        if self
            .journey
            .as_ref()
            .is_some_and(|previous| previous.revision > journey.revision)
            || workspace.forms.iter().enumerate().any(|(index, form)| {
                workspace.forms[..index]
                    .iter()
                    .any(|prior| prior.form == form.form)
            })
        {
            return Err(Error::Presentation);
        }
        let selected = workspace
            .forms
            .iter()
            .find(|form| form.foreground)
            .ok_or(Error::Presentation)?;
        if selected.form.source_document_id != journey.source_document_id
            || selected.form.checked_form_id != journey.checked_form_id
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
            // This native slice has an immutable resident workset. Revisions
            // change selection/output; they do not re-check Forms during Play.
            if previous.forms.len() != workspace.forms.len()
                || previous
                    .forms
                    .iter()
                    .zip(&workspace.forms)
                    .any(|(left, right)| left.form != right.form || left.title != right.title)
            {
                return Err(Error::Presentation);
            }
        } else {
            for form in &workspace.forms {
                let kind =
                    crate::native_workset::resolve(&form.form).map_err(|_| Error::Presentation)?;
                if kind.title() != form.title {
                    return Err(Error::Presentation);
                }
            }
        }
        self.revision.checked_add(1).ok_or(Error::Presentation)?;
        self.source_document_id = journey.source_document_id.clone();
        self.checked_form_id = journey.checked_form_id.clone();
        self.form_subject = format!("form/{}", self.checked_form_id.as_str());
        if self.selected_subject.starts_with("form/") {
            self.selected_subject = self.form_subject.clone();
        }
        self.form_open = false;
        // A newly admitted Play supersedes an earlier startup/input refusal.
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
