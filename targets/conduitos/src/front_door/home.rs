//! Finite ConduitOS Home and command-bar face state.

use alloc::vec::Vec;
pub use conduit_home_model::{HomeAction as HomeInput, HomeView};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_home_model::{HomeEvent, HomeModel};
use conduit_human::KeyEvent;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_human::{KeyTransition, PreparedKeymap};

use super::{Error, FrontDoor};
use crate::product_journey::JourneyStatus;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
mod presentation;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod scene;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
pub(super) use crate::protected_creche::Home;

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
pub(super) struct Home {
    model: HomeModel,
    pub(super) commons: super::commons::Commons,
    keymap: PreparedKeymap,
}

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
impl Home {
    fn new() -> Result<Self, Error> {
        Ok(Self {
            commons: super::commons::Commons::prepare()?,
            model: HomeModel::plots(),
            keymap: PreparedKeymap::default_layout(),
        })
    }

    fn try_new() -> Result<Self, Error> {
        Self::new()
    }

    fn reset(&mut self) -> Result<(), Error> {
        self.model = HomeModel::plots();
        Ok(())
    }

    fn model(&self) -> &HomeModel {
        &self.model
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
            41 => return HomeInput::Unchanged,
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

    pub(super) fn home_active_plots(&self) -> &[crate::product_journey::WorkspacePlot] {
        if self.journey.as_ref().is_some_and(|journey| {
            journey.active_play_id.is_some()
                && journey.status == crate::product_journey::JourneyStatus::QuiescentAwaitingInput
        }) {
            self.home_installed_plots()
        } else {
            &[]
        }
    }

    fn home_installed_titles(&self) -> Vec<&'static str> {
        self.home_active_plots()
            .iter()
            .map(|plot| plot.title)
            .collect()
    }

    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    pub(super) fn home_model(&self) -> Option<&HomeModel> {
        self.home.as_ref().map(|home| &home.model)
    }

    pub fn open_home(&mut self) -> Result<(), Error> {
        if self.journey.is_none() {
            return Err(Error::Face);
        }
        if self.home.is_some() {
            return self.advance();
        }
        let home = if let Some(mut retained) = self.retained_home.take() {
            retained
                .commons
                .back_to_hub(self.journey.as_ref().ok_or(Error::Face)?)?;
            retained.reset()?;
            retained
        } else {
            Home::try_new()?
        };
        self.home = Some(home);
        self.advance()
    }

    /// Move, rather than copy, the Commons context while its Birth encounter is visible.
    #[cfg_attr(not(feature = "native-compositor"), allow(dead_code))]
    pub(crate) fn enter_prebirth_creche(&mut self) -> Result<(), Error> {
        if !self.commons_creche_selected()
            || self.arrival.is_none()
            || self.retained_home.is_some()
            || self
                .journey
                .as_ref()
                .is_none_or(|journey| journey.body_id.is_some())
        {
            return Err(Error::Face);
        }
        self.retained_home = self.home.take();
        self.advance()
    }

    pub fn commons_creche_selected(&self) -> bool {
        self.home.as_ref().is_some_and(|home| home.commons.creche())
    }

    pub fn commons_place(&self) -> Option<&str> {
        self.home.as_ref().map(|home| home.commons.selected())
    }

    pub fn home_open(&self) -> bool {
        self.home.is_some()
    }

    pub fn close_home(&mut self) -> Result<(), Error> {
        if self.retained_home.is_some() || self.home.is_none() {
            return Err(Error::Face);
        }
        self.retained_home = self.home.take();
        self.advance()
    }

    pub fn home_view(&self) -> Option<HomeView> {
        self.home.as_ref().map(|home| home.model().view())
    }

    pub fn home_selection(&self) -> Option<usize> {
        self.home
            .as_ref()
            .map(|home| home.model().selected_plot_index())
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
        let journey = self.journey.as_ref().ok_or(Error::Face)?;
        let result = if journey.body_id.is_none() {
            home.commons.accept_birth(event, journey)?
        } else if journey.active_play_id.is_some()
            && journey.status == JourneyStatus::QuiescentAwaitingInput
        {
            home.accept_checked(event, &titles)?
        } else {
            HomeInput::Unchanged
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
    fn picker_cycles_only_the_provided_residents_and_has_no_second_launcher() {
        let mut home = Home::new().unwrap();
        assert_eq!(home.model().view(), HomeView::Plots);
        for index in 0..5 {
            assert_eq!(
                home.accept(key(40), &["Text editor", "Canvas"]),
                HomeInput::OpenPlot(index % 2)
            );
            assert_eq!(
                home.accept(key(43), &["Text editor", "Canvas"]),
                HomeInput::Changed
            );
        }
        assert_eq!(home.accept(key(41), &[]), HomeInput::Unchanged);
        assert_eq!(home.model().view(), HomeView::Plots);
        assert_eq!(home.accept(key(40), &[]), HomeInput::Unchanged);
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
