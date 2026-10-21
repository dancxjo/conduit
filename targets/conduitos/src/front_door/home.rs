//! Finite ConduitOS Home and command-bar face state.

use alloc::{format, string::String, vec::Vec};
pub use conduit_home_model::{HomeAction as HomeInput, HomeView};
use conduit_home_model::{HomeEvent, HomeModel};
use conduit_human::{KeyEvent, KeyTransition, PreparedKeymap};

use super::{Error, FrontDoor};
mod scene;

pub(super) struct Home {
    model: HomeModel,
    pub(super) commons: super::commons::Commons,
    keymap: PreparedKeymap,
    refusal: Option<String>,
}

impl Home {
    fn new() -> Result<Self, Error> {
        Ok(Self {
            commons: super::commons::Commons::prepare()?,
            model: HomeModel::new(),
            keymap: PreparedKeymap::default_layout(),
            refusal: None,
        })
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
        self.home.as_ref().and_then(|home| home.refusal.as_deref())
    }

    pub(super) fn home_model(&self) -> Option<&HomeModel> {
        self.home.as_ref().map(|home| &home.model)
    }

    pub fn open_home(&mut self) -> Result<(), Error> {
        if self
            .journey
            .as_ref()
            .is_none_or(|journey| journey.body_id.is_none())
        {
            return Err(Error::Face);
        }
        self.home = Some(Home::new()?);
        self.advance()
    }

    pub fn commons_place(&self) -> Option<&str> {
        self.home.as_ref().map(|home| home.commons.selected())
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
        self.home.as_ref().map(|home| home.model.view())
    }

    pub fn home_selection(&self) -> Option<usize> {
        self.home.as_ref().map(|home| home.model.selected_index())
    }

    pub fn home_application_view(&self) -> Result<Option<conduit_face::ApplicationView>, Error> {
        self.home
            .as_ref()
            .map(|home| {
                home.model
                    .face(
                        u32::try_from(self.revision).map_err(|_| Error::Face)?,
                        &self.home_installed_titles(),
                    )
                    .lower()
                    .map_err(|_| Error::Face)
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
        let result = match home.commons.accept(
            event,
            home.model.view(),
            self.journey.as_ref().ok_or(Error::Face)?,
        )? {
            Some(result) => result,
            None => home.accept(event, &titles),
        };
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
        let result = if let Some(title) = absent {
            home.refusal = Some(format!(
                "{title} is unavailable: this Body has no installed {title} Plot."
            ));
            HomeInput::Changed
        } else {
            if result != HomeInput::Unchanged {
                home.refusal = None;
            }
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
        let mut home = Home::new().unwrap();
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
