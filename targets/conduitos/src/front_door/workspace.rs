//! Exact body membership and foreground view, supplied by ProductJourney.
use super::{Error, FrontDoor};
use crate::product_journey::{JourneyProjection, WorkspaceProjection};
use alloc::{format, string::String};

pub(super) enum WorkspaceRefusal {
    Startup(String),
    Play(String),
    Action(String),
}
impl WorkspaceRefusal {
    pub(super) fn reason(&self) -> &str {
        match self {
            Self::Startup(reason) | Self::Play(reason) | Self::Action(reason) => reason,
        }
    }
    pub(super) fn key(&self) -> &'static str {
        match self {
            Self::Startup(_) => "startup-refusal",
            Self::Play(_) => "play-refusal",
            Self::Action(_) => "action-refusal",
        }
    }
    #[allow(dead_code)]
    pub(super) fn heading(&self) -> &'static str {
        match self {
            Self::Startup(_) => "Startup could not finish. Details:",
            Self::Play(_) => "Play stopped. Details:",
            Self::Action(_) => "Action refused. Details:",
        }
    }
}

impl FrontDoor {
    pub fn inspect_lifecycle(&mut self) -> Result<(), Error> {
        self.revision.checked_add(1).ok_or(Error::Face)?;
        self.reading_command(conduitos_mask_core::control::Operation::Inspect)?;
        self.advance()
    }

    /// Whether the rendered encounter still observes this exact owner state.
    /// Checking does not publish a new revision or acknowledge another Show.
    #[cfg(all(target_arch = "x86_64", feature = "native-compositor"))]
    pub(crate) fn observes_current_product(
        &self,
        journey: &crate::product_journey::ProductJourney,
    ) -> bool {
        self.journey.as_ref() == Some(&journey.projection())
            && self.workspace == journey.workspace_projection()
    }

    /// Refresh from the authoritative lifecycle and its exact resident workset.
    pub fn observe_product(
        &mut self,
        journey: &crate::product_journey::ProductJourney,
    ) -> Result<(), Error> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            self.graph_facts = None;
        }
        if let Some(workspace) = journey.workspace_projection() {
            let application_view = if journey.foreground_is_tutorial() {
                Some(journey.tutorial_view().map_err(|_| Error::Face)?)
            } else {
                journey.foreground_application_view().cloned()
            };
            if let Some(view) = &application_view {
                view.validate().map_err(|_| Error::Face)?;
            }
            let changed = self.application_view != application_view
                || self
                    .workspace
                    .as_ref()
                    .and_then(|prior| prior.plots.iter().find(|plot| plot.foreground))
                    .map(|plot| &plot.plot)
                    != workspace
                        .plots
                        .iter()
                        .find(|plot| plot.foreground)
                        .map(|plot| &plot.plot);
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            let graph_facts = journey
                .foreground_patchbay_graph()
                .map(|graph| {
                    // The outer Face has the foreground Plot subject only;
                    // other installed members are not already Face subjects.
                    let present = workspace
                        .plots
                        .iter()
                        .find(|plot| plot.foreground)
                        .is_some_and(|plot| plot.plot.checked_plot_id == graph.checked_plot_id);
                    journey
                        .graph_face_facts(graph, present)
                        .map_err(|_| Error::Face)
                })
                .transpose()?;
            self.observe_body(journey.projection(), workspace)?;
            #[cfg(all(target_arch = "x86_64", target_os = "none"))]
            {
                self.graph_facts = graph_facts;
            }
            if changed {
                self.reset_application_navigation();
            }
            self.application_view = application_view;
            self.patchbay_graph = journey.foreground_patchbay_graph().cloned();
            Ok(())
        } else {
            self.application_view = None;
            self.patchbay_graph = None;
            self.observe_journey(journey.projection())
        }
    }

    #[cfg_attr(not(feature = "native-compositor"), allow(dead_code))]
    pub(crate) fn action_refused(&mut self, reason: &str) -> Result<(), Error> {
        self.refusal = Some(WorkspaceRefusal::Action(reason.into()));
        self.advance()
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
            return Err(Error::Face);
        }
        self.refusal = Some(WorkspaceRefusal::Play(reason.into()));
        self.advance()
    }

    pub fn observe_body(
        &mut self,
        journey: JourneyProjection,
        workspace: WorkspaceProjection,
    ) -> Result<(), Error> {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        {
            self.graph_facts = None;
        }
        if journey.body_id.as_ref() != Some(&workspace.body_id)
            || journey.revision != workspace.revision
            || workspace.plots.is_empty()
            || workspace.plots.len() > crate::native_workset::NATIVE_PLOT_CAPACITY
            || workspace
                .plots
                .windows(2)
                .any(|pair| pair[0].plot >= pair[1].plot)
            || workspace
                .plots
                .iter()
                .filter(|plot| plot.foreground)
                .count()
                != 1
        {
            return Err(Error::Face);
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
            return Err(Error::Face);
        }
        let selected = workspace
            .plots
            .iter()
            .find(|plot| plot.foreground)
            .ok_or(Error::Face)?;
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
            return Err(Error::Face);
        }
        if let Some(previous) = &self.workspace {
            let unchanged = previous.plots.len() == workspace.plots.len()
                && previous
                    .plots
                    .iter()
                    .zip(&workspace.plots)
                    .all(|(left, right)| left.plot == right.plot && left.title == right.title);
            let admitted_insert = workspace.plots.len() == previous.plots.len() + 1
                && previous.plots.iter().all(|prior| {
                    workspace
                        .plots
                        .iter()
                        .any(|current| current.plot == prior.plot && current.title == prior.title)
                })
                && self.journey.as_ref().is_some_and(|prior| {
                    prior
                        .workload_revision
                        .zip(journey.workload_revision)
                        .is_some_and(|(left, right)| left.checked_add(1) == Some(right))
                })
                && journey.workload_sign_id.is_some();
            // Selection and output cannot rewrite membership. The sole growth
            // case is one exact canonical insertion backed by the Body/Wake event.
            if !unchanged && !admitted_insert {
                return Err(Error::Face);
            }
            if admitted_insert {
                let inserted = workspace
                    .plots
                    .iter()
                    .find(|current| {
                        !previous
                            .plots
                            .iter()
                            .any(|prior| prior.plot == current.plot)
                    })
                    .ok_or(Error::Face)?;
                let kind =
                    crate::native_workset::resolve(&inserted.plot).map_err(|_| Error::Face)?;
                if kind.title() != inserted.title {
                    return Err(Error::Face);
                }
            }
        } else {
            for plot in &workspace.plots {
                let kind = crate::native_workset::resolve(&plot.plot).map_err(|_| Error::Face)?;
                if kind.title() != plot.title {
                    return Err(Error::Face);
                }
            }
        }
        self.revision.checked_add(1).ok_or(Error::Face)?;
        self.reading_command(conduitos_mask_core::control::Operation::BodyAdmitted)?;
        if self
            .journey
            .as_ref()
            .is_some_and(|prior| prior.revision != journey.revision)
            && matches!(self.refusal, Some(WorkspaceRefusal::Action(_)))
        {
            self.refusal = None;
        }
        self.source_document_id = journey.source_document_id.clone().ok_or(Error::Face)?;
        self.checked_plot_id = journey.checked_plot_id.clone().ok_or(Error::Face)?;
        self.plot_subject = format!("plot/{}", self.checked_plot_id.as_str());
        // A newly admitted play supersedes an earlier startup/input refusal.
        if journey.status == crate::product_journey::JourneyStatus::QuiescentAwaitingInput
            && !matches!(self.refusal, Some(WorkspaceRefusal::Action(_)))
        {
            self.refusal = None;
        }
        self.journey = Some(journey);
        self.workspace = Some(workspace);
        self.advance()
    }
}

#[cfg(test)]
mod tests;
