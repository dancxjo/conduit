mod executor;
mod journey;
mod layout;
mod patchbay;
mod presentation;

pub use executor::*;
pub use journey::*;
pub use layout::*;
pub use patchbay::*;
pub use presentation::*;

pub const INSTALLED_FORMS: [&str; 4] = ["Hello", "Text Lab", "Clock", "Count"];

use conduit_home_model::{HomeAction, HomeEvent, HomeModel};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeHomeRequest {
    OpenTour,
    OpenPatchbay,
    OpenCreche,
    OpenForm(usize),
    RunForm(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHomeController {
    model: HomeModel,
    revision: u32,
    notice: String,
}

impl Default for NativeHomeController {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeHomeController {
    pub fn new() -> Self {
        Self {
            model: HomeModel::new(),
            revision: 1,
            notice: "Arrow keys move  |  Enter opens  |  Type for Prompt  |  Esc returns Home"
                .into(),
        }
    }

    pub const fn model(&self) -> &HomeModel {
        &self.model
    }

    pub const fn revision(&self) -> u32 {
        self.revision
    }

    pub fn notice(&self) -> &str {
        &self.notice
    }

    pub fn presentation(&self) -> Result<NativeHomePresentation, NativeHomeRefusal> {
        NativeHomePresentation::project(&self.model, self.revision, &INSTALLED_FORMS)
    }

    pub fn accept(&mut self, event: HomeEvent<'_>) -> Option<NativeHomeRequest> {
        let action = self.model.accept(event, &INSTALLED_FORMS);
        self.finish(action)
    }

    pub fn submit_text(&mut self, text: &str) -> Option<NativeHomeRequest> {
        let action = self.model.submit_text(text, &INSTALLED_FORMS);
        self.finish(action)
    }

    pub fn activate_index(&mut self, index: usize) -> Option<NativeHomeRequest> {
        let count = match self.model.view() {
            conduit_home_model::HomeView::Launcher => conduit_home_model::HOME_ITEM_COUNT,
            conduit_home_model::HomeView::Forms => INSTALLED_FORMS.len(),
            _ => return None,
        };
        if index >= count {
            return None;
        }
        while match self.model.view() {
            conduit_home_model::HomeView::Launcher => self.model.selected_index(),
            conduit_home_model::HomeView::Forms => self.model.selected_form_index(),
            _ => return None,
        } != index
        {
            let _ = self.model.accept(HomeEvent::Next, &INSTALLED_FORMS);
        }
        self.bump_revision();
        self.accept(HomeEvent::Activate)
    }

    pub fn report_request(&mut self, request: &NativeHomeRequest, outcome: Result<(), &str>) {
        self.notice = match outcome {
            Ok(()) => match request {
                NativeHomeRequest::OpenTour => "Tour opened on this host.".into(),
                NativeHomeRequest::OpenPatchbay => "Patchbay opened on this host.".into(),
                NativeHomeRequest::OpenCreche => "Crèche opened on this host.".into(),
                NativeHomeRequest::OpenForm(index) => {
                    format!("Selected form {}.", INSTALLED_FORMS[*index])
                }
                NativeHomeRequest::RunForm(index) => {
                    format!("Completed Form {} on this host.", INSTALLED_FORMS[*index])
                }
            },
            Err(error) => format!("Host refused the request: {error}"),
        };
        self.bump_revision();
    }

    fn finish(&mut self, action: HomeAction) -> Option<NativeHomeRequest> {
        let request = match action {
            HomeAction::Unchanged => return None,
            HomeAction::Changed => {
                self.bump_revision();
                return None;
            }
            HomeAction::OpenTour => NativeHomeRequest::OpenTour,
            HomeAction::OpenPatchbay => NativeHomeRequest::OpenPatchbay,
            HomeAction::OpenCreche => NativeHomeRequest::OpenCreche,
            HomeAction::OpenForm(index) => NativeHomeRequest::OpenForm(index),
            HomeAction::RunForm(index) => NativeHomeRequest::RunForm(index),
        };
        self.bump_revision();
        Some(request)
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
    }
}
