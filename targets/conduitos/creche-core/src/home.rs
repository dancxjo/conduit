//! Protected native Home controller around the canonical HomeModel.
pub mod data;
use crate::wire::Refusal;
use alloc::{format, string::String};
use conduit_home_model::{HomeAction, HomeEvent, HomeModel, HomeView};
use conduit_human::{KeyEvent, KeyTransition, PreparedKeymap};
pub(crate) struct Controller {
    model: HomeModel,
    keymap: PreparedKeymap,
    refusal: Option<String>,
    revision: u32,
}
impl Controller {
    pub(crate) fn new() -> Self {
        Self {
            model: HomeModel::new(),
            keymap: PreparedKeymap::default_layout(),
            refusal: None,
            revision: 1,
        }
    }
    pub(crate) fn reset(&mut self) -> Result<(), Refusal> {
        let next = self
            .revision
            .checked_add(1)
            .ok_or(Refusal::RevisionExhausted)?;
        *self = Self::new();
        self.revision = next;
        Ok(())
    }
    pub(crate) fn revision(&self) -> u32 {
        self.revision
    }
    pub(crate) fn snapshot(&self) -> data::Snapshot {
        data::Snapshot {
            revision: self.revision,
            view: match self.model.view() {
                HomeView::Launcher => 0,
                HomeView::Plots => 1,
                HomeView::Body => 2,
                HomeView::Prompt => 3,
            },
            selected: self.model.selected_index() as u8,
            plot_selected: self.model.selected_plot_index() as u8,
            command: self.model.command().into(),
            output: self.model.output().into(),
            refusal: self.refusal.clone(),
        }
    }
    pub(crate) fn reply(&self, action: HomeAction) -> data::Reply {
        let (action, index) = match action {
            HomeAction::Unchanged => (0, 0),
            HomeAction::Changed => (1, 0),
            HomeAction::OpenTour => (2, 0),
            HomeAction::OpenPatchbay => (3, 0),
            HomeAction::OpenCreche => (4, 0),
            HomeAction::OpenPlot(index) => (5, index as u32),
            HomeAction::RunPlot(index) => (6, index as u32),
        };
        data::Reply {
            snapshot: self.snapshot(),
            action,
            index,
        }
    }
    pub(crate) fn face(
        &self,
        revision: u32,
        plots: &[&str],
    ) -> conduit_face::SemanticApplicationView {
        self.model.face(revision, plots)
    }
    pub(crate) fn refuse(&mut self, code: u8) -> Result<(), Refusal> {
        let title = match code {
            1 => "Tour",
            2 => "Patchbay",
            _ => return Err(Refusal::Malformed),
        };
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(Refusal::RevisionExhausted)?;
        self.refusal = Some(format!(
            "{title} is unavailable: this Body has no installed {title} Plot."
        ));
        self.revision = revision;
        Ok(())
    }
    pub(crate) fn accept(
        &mut self,
        event: KeyEvent,
        plots: &[&str],
    ) -> Result<HomeAction, Refusal> {
        // Reserve the local revision fence before the canonical controller can mutate.
        let next = self
            .revision
            .checked_add(1)
            .ok_or(Refusal::RevisionExhausted)?;
        let result = self.accept_key(event, plots);
        if result != HomeAction::Unchanged {
            self.refusal = None;
            self.revision = next;
        }
        Ok(result)
    }
    fn accept_key(&mut self, event: KeyEvent, plots: &[&str]) -> HomeAction {
        if event.transition() != KeyTransition::Pressed {
            let _ = self.keymap.apply(event);
            return HomeAction::Unchanged;
        }
        let abstract_event = match event.usage() {
            79 | 81 | 43 => HomeEvent::Next,
            80 | 82 => HomeEvent::Previous,
            40 => HomeEvent::Activate,
            41 => HomeEvent::Escape,
            42 => HomeEvent::Backspace,
            _ => match self.keymap.apply(event) {
                Ok(Some(fragment)) => {
                    let text = fragment.as_slice();
                    return core::str::from_utf8(text).map_or(HomeAction::Unchanged, |text| {
                        self.model.accept(HomeEvent::Text(text), plots)
                    });
                }
                Err(_) => HomeEvent::CharacterUnavailable,
                _ => return HomeAction::Unchanged,
            },
        };
        self.model.accept(abstract_event, plots)
    }
}

pub mod presentation;
mod projection;
mod scene;
