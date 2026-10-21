//! Finite ConduitOS Home and command-bar face state.

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub(super) use crate::protected_creche::Home;
use alloc::{format, string::String, vec::Vec};
pub use conduit_home_model::{HomeAction as HomeInput, HomeView};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_home_model::{HomeEvent, HomeModel};
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
type HomeReading = conduitos_creche_core::home::data::Snapshot;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
type HomeReading = HomeModel;
mod scene;
use conduit_human::KeyEvent;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_human::{KeyTransition, PreparedKeymap};

use super::{Error, FrontDoor};

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub(super) struct Home {
    model: HomeModel,
    keymap: PreparedKeymap,
    refusal: Option<String>,
}

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
impl Home {
    fn new() -> Self {
        Self {
            model: HomeModel::new(),
            keymap: PreparedKeymap::default_layout(),
            refusal: None,
        }
    }

    fn try_new() -> Result<Self, Error> {
        Ok(Self::new())
    }
    fn reset(&mut self) -> Result<(), Error> {
        *self = Self::new();
        Ok(())
    }
    fn is_live(&self) -> bool {
        true
    }
    fn model(&self) -> &HomeModel {
        &self.model
    }
    fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }
    fn application_view(
        &self,
        revision: u32,
        titles: &[&str],
    ) -> Result<conduit_face::ApplicationView, Error> {
        self.model
            .face(revision, titles)
            .lower()
            .map_err(|_| Error::Face)
    }
    fn record_result(&mut self, result: &HomeInput, absent: Option<&str>) -> Result<(), Error> {
        if let Some(title) = absent {
            self.refusal = Some(format!(
                "{title} is unavailable: this Body has no installed {title} Plot."
            ));
        } else if *result != HomeInput::Unchanged {
            self.refusal = None;
        }
        Ok(())
    }
    fn accept_checked(&mut self, event: KeyEvent, plots: &[&str]) -> Result<HomeInput, Error> {
        Ok(self.accept(event, plots))
    }
    fn accept(&mut self, event: KeyEvent, plots: &[&str]) -> HomeInput {
        if event.transition() != KeyTransition::Pressed {
            let _ = self.keymap.apply(event);
            return HomeInput::Unchanged;
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
                    return core::str::from_utf8(text).map_or(HomeInput::Unchanged, |text| {
                        self.model.accept(HomeEvent::Text(text), plots)
                    });
                }
                Err(_) => HomeEvent::CharacterUnavailable,
                _ => return HomeInput::Unchanged,
            },
        };
        self.model.accept(abstract_event, plots)
    }
}

impl FrontDoor {
    /// Exact observed Body membership, distinct from the Host's available catalog.
    pub fn home_installed_plots(&self) -> &[crate::product_journey::WorkspacePlot] {
        self.workspace
            .as_ref()
            .map_or(&[], |workspace| workspace.plots.as_slice())
    }

    fn home_installed_titles(&self) -> Vec<&'static str> {
        self.home_installed_plots()
            .iter()
            .map(|plot| plot.title)
            .collect()
    }

    pub(super) fn home_refusal(&self) -> Option<&str> {
        self.home.as_ref().and_then(|home| home.refusal())
    }

    pub(super) fn home_model(&self) -> Option<&HomeReading> {
        self.home
            .as_ref()
            .filter(|home| home.is_live())
            .map(|home| home.model())
    }

    pub fn open_home(&mut self) -> Result<(), Error> {
        if self
            .journey
            .as_ref()
            .is_none_or(|journey| journey.body_id.is_none())
        {
            return Err(Error::Face);
        }
        if let Some(home) = self.home.as_mut().filter(|home| home.is_live()) {
            home.reset()?;
        } else {
            // Explicit lifecycle reentry drops an inert old owner before a fresh install.
            self.home.take();
            self.home = Some(Home::try_new()?);
        }
        self.advance()
    }

    pub fn home_open(&self) -> bool {
        self.home.is_some()
    }

    pub fn close_home(&mut self) -> Result<(), Error> {
        if self.home.take().is_none() {
            return Err(Error::Face);
        }
        self.advance()
    }

    pub fn home_view(&self) -> Option<HomeView> {
        self.home
            .as_ref()
            .filter(|home| home.is_live())
            .map(|home| home.model().view())
    }

    pub fn home_selection(&self) -> Option<usize> {
        self.home
            .as_ref()
            .filter(|home| home.is_live())
            .map(|home| home.model().selected_index())
    }

    pub fn home_application_view(&self) -> Result<Option<conduit_face::ApplicationView>, Error> {
        self.home
            .as_ref()
            .map(|home| {
                home.application_view(
                    u32::try_from(self.revision).map_err(|_| Error::Face)?,
                    &self.home_installed_titles(),
                )
            })
            .transpose()
    }

    pub fn accept_home(&mut self, event: KeyEvent, revision: u64) -> Result<HomeInput, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        let titles = self.home_installed_titles();
        let Some(home) = self.home.as_mut() else {
            return Ok(HomeInput::Unchanged);
        };
        let result = home.accept_checked(event, &titles)?;
        if matches!(result,HomeInput::OpenPlot(index)|HomeInput::RunPlot(index) if index>=titles.len())
        {
            return Err(Error::ActionUnavailable);
        }
        let absent = match result {
            HomeInput::OpenTour
                if !titles.contains(&crate::native_workset::NativePlot::Tour.title()) =>
            {
                Some("Tour")
            }
            HomeInput::OpenPatchbay
                if !titles.contains(&crate::native_workset::NativePlot::Patchbay.title()) =>
            {
                Some("Patchbay")
            }
            _ => None,
        };
        home.record_result(&result, absent)?;
        let result = if absent.is_some() {
            HomeInput::Changed
        } else {
            result
        };
        if result != HomeInput::Unchanged {
            self.advance()?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_human::{KeyModifiers, KeyTransition};

    fn key(usage: u8) -> KeyEvent {
        KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap()
    }

    #[test]
    fn launcher_is_finite_and_opens_prompt() {
        let mut home = Home::new();
        for _ in 0..5 {
            assert_eq!(home.accept(key(43), &[]), HomeInput::Changed);
        }
        assert_eq!(home.accept(key(40), &[]), HomeInput::Changed);
        assert_eq!(home.model.view(), HomeView::Prompt);
    }

    #[test]
    fn command_vocabulary_refuses_unknown_text() {
        let mut model = HomeModel::new();
        assert_eq!(
            model.submit_text("open nowhere", &["Tour"]),
            HomeInput::Changed
        );
        assert_eq!(model.output(), "Unknown command: open nowhere");
        assert_eq!(
            model.submit_text("open tour", &["Tour"]),
            HomeInput::OpenTour
        );
    }
}
