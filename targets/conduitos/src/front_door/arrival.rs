//! Native input adaptation for the shared Crèche draft; no lifecycle authority.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
use crate::protected_creche::CrecheDraft as BirthDraft;
use alloc::string::String;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use alloc::{format, vec::Vec};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_birth_plot::BirthDraft;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_birth_plot::BirthDraftRefusal;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_birth_plot::{BirthActionOutcome, BirthActions, BirthWidgetFace};
use conduit_birth_plot::{BirthPlotChoice, BirthSelection};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_face::ApplicationEventKind;
use conduit_human::{KeyEvent, KeyTransition};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_human::{KeyModifiers, PreparedKeymap};

use super::{Error, FrontDoor};

mod mask_interaction;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod projection;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
mod protected_projection;
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
mod scene;

pub(super) struct Arrival {
    draft: BirthDraft,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    focus: usize,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    replace_name: bool,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    keymap: PreparedKeymap,
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    refusal: Option<String>,
}

pub enum ArrivalInput {
    Unchanged,
    Changed,
    Birth(BirthSelection),
}

impl FrontDoor {
    pub fn open_creche(&mut self, uuid: String, refusal: Option<String>) -> Result<(), Error> {
        self.open_creche_reviewed(uuid, core::array::from_fn(|_| refusal.clone()))
    }

    pub fn open_creche_reviewed(
        &mut self,
        uuid: String,
        refusals: [Option<String>; crate::native_workset::NATIVE_CANDIDATE_CAPACITY],
    ) -> Result<(), Error> {
        if !self.lifecycle_authority_admitted {
            return Err(Error::ActionUnavailable);
        }
        if self.arrival.is_some() || self.journey.as_ref().is_some_and(|j| j.body_id.is_some()) {
            return Err(Error::Face);
        }
        let draft = BirthDraft::new(
            uuid,
            crate::native_workset::candidate_inventory()
                .into_iter()
                .zip(refusals)
                .map(|(plot, refusal)| {
                    // A visible Source candidate is not installed authority.
                    // Even a missing caller refusal cannot widen this profile.
                    let refusal = if crate::native_workset::profile().installed().contains(&plot) {
                        refusal
                    } else {
                        Some(
                            crate::native_workset::WorksetRefusal::UnknownPlot
                                .as_str()
                                .into(),
                        )
                    };
                    let available = refusal.is_none();
                    Ok(BirthPlotChoice {
                        title: plot.title().into(),
                        search_text: plot.source().into(),
                        plot: crate::native_workset::resident(plot).map_err(|_| Error::Face)?,
                        refusal,
                        selected: available,
                    })
                })
                .collect::<Result<_, Error>>()?,
        )
        .map_err(|_| Error::Face)?;
        self.arrival = Some(Arrival {
            draft,
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            focus: 0,
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            replace_name: true,
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            keymap: PreparedKeymap::default_layout(),
            #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
            refusal: None,
        });
        self.advance()
    }

    pub fn creche_open(&self) -> bool {
        self.arrival.is_some() && self.home.is_none()
    }

    pub fn close_creche(&mut self) -> Result<(), Error> {
        if self.journey.as_ref().is_none_or(|j| j.body_id.is_none()) {
            return Err(Error::Face);
        }
        self.arrival = None;
        self.advance()
    }

    pub fn accept_creche(&mut self, event: KeyEvent, revision: u64) -> Result<ArrivalInput, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        if event.transition() == KeyTransition::Pressed {
            self.revision.checked_add(1).ok_or(Error::Face)?;
        }
        let Some(arrival) = self.arrival.as_mut() else {
            return Ok(ArrivalInput::Unchanged);
        };
        let outcome = arrival.accept(event);
        if !matches!(outcome, ArrivalInput::Unchanged) {
            self.advance()?;
        }
        Ok(outcome)
    }
}

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
impl Arrival {
    fn accept(&mut self, event: KeyEvent) -> ArrivalInput {
        if event.transition() != KeyTransition::Pressed {
            // Preserve portable transition handling while a dead key is pending.
            let _ = self.keymap.apply(event);
            return ArrivalInput::Unchanged;
        }
        let controls = self.controls();
        let Some(action) = controls.get(self.focus) else {
            return ArrivalInput::Unchanged;
        };
        let composing = self.keymap.is_composing();
        let result = match event.usage() {
            43 | 81 | 82 => {
                let backward = event.usage() == 82
                    || (event.usage() == 43
                        && event.modifiers_after().bits()
                            & (KeyModifiers::LEFT_SHIFT.bits() | KeyModifiers::RIGHT_SHIFT.bits())
                            != 0);
                let count = controls.len();
                self.focus = (self.focus + if backward { count - 1 } else { 1 }) % count;
                self.replace_name = self.focus == 0;
                self.keymap.reset();
                Ok(())
            }
            59 => self.suggest(), // F2: the same suggestion action as the Crèche button.
            79 | 80 if self.focus == 1 => self.cycle_tradition(event.usage() == 80),
            60 => return self.submit(),
            40 if action == "creche.name" => match self.keymap.apply(event) {
                Ok(Some(fragment)) if !composing && fragment.as_slice() == b"\n" => {
                    return self.submit();
                }
                Ok(Some(fragment)) => self.append_name(fragment.as_slice()),
                Err(_) => {
                    self.refusal = Some("The keyboard could not complete that character.".into());
                    return ArrivalInput::Changed;
                }
                _ => return ArrivalInput::Unchanged,
            },
            40 if action == "creche.birth" => {
                return self.submit();
            }
            40 | 44 if self.focus == 2 => self.suggest(),
            40 | 44 if self.focus == 1 => self.cycle_tradition(false),
            40 | 44 if action.starts_with("creche.plot.") => {
                let Some(index) = action
                    .strip_prefix("creche.plot.")
                    .and_then(|value| value.parse::<usize>().ok())
                else {
                    return ArrivalInput::Unchanged;
                };
                let value = if self.draft.choices()[index].selected {
                    "false"
                } else {
                    "true"
                };
                self.change(
                    &format!("creche.plot.{index}"),
                    ApplicationEventKind::Change,
                    value,
                )
            }
            42 if action == "creche.search" => {
                let mut search = String::from(self.draft.search());
                search.pop();
                self.change("creche.search", ApplicationEventKind::Input, &search)
            }
            _ if action == "creche.search" => match self.keymap.apply(event) {
                Ok(Some(fragment)) => {
                    let mut search = String::from(self.draft.search());
                    search.push_str(core::str::from_utf8(fragment.as_slice()).unwrap());
                    self.change("creche.search", ApplicationEventKind::Input, &search)
                }
                _ => return ArrivalInput::Unchanged,
            },
            42 if self.focus == 0 => {
                let mut name = self.draft.friendly_name().into();
                if self.replace_name {
                    name = String::new();
                } else {
                    String::pop(&mut name);
                }
                self.replace_name = false;
                self.change("creche.name", ApplicationEventKind::Input, &name)
            }
            _ if self.focus == 0 => match self.keymap.apply(event) {
                Ok(Some(fragment)) => self.append_name(fragment.as_slice()),
                _ => return ArrivalInput::Unchanged,
            },
            _ => return ArrivalInput::Unchanged,
        };
        self.refusal = result.err().map(|error| refusal_text(error).into());
        ArrivalInput::Changed
    }

    fn append_name(&mut self, fragment: &[u8]) -> Result<(), BirthDraftRefusal> {
        let text = core::str::from_utf8(fragment).map_err(|_| BirthDraftRefusal::InvalidName)?;
        let mut name = if self.replace_name {
            String::new()
        } else {
            self.draft.friendly_name().into()
        };
        name.push_str(text);
        self.change("creche.name", ApplicationEventKind::Input, &name)?;
        self.replace_name = false;
        Ok(())
    }

    fn controls(&self) -> Vec<String> {
        use conduit_face::FaceMechanism;
        let Ok(view) = self.draft.widget_face() else {
            return Vec::new();
        };
        let mut controls = Vec::new();
        for node in &view.root.children {
            match &node.mechanism {
                FaceMechanism::FormField(field) => {
                    controls.push(field.input_action.identity.clone())
                }
                FaceMechanism::Action(action) => controls.push(action.identity.clone()),
                FaceMechanism::ChoiceGroup { options, .. } => controls.extend(
                    options
                        .iter()
                        .map(|choice| choice.change_action.identity.clone()),
                ),
                _ => {}
            }
        }
        controls
    }

    fn submit(&mut self) -> ArrivalInput {
        match self.draft.apply_event(
            self.draft.revision(),
            "creche.birth",
            ApplicationEventKind::Activate,
            "",
        ) {
            Ok(BirthActionOutcome::Birth(selection)) => {
                self.refusal = None;
                ArrivalInput::Birth(selection)
            }
            Ok(BirthActionOutcome::Changed) => ArrivalInput::Changed,
            Err(error) => {
                self.refusal = Some(refusal_text(error).into());
                ArrivalInput::Changed
            }
        }
    }
    fn suggest(&mut self) -> Result<(), BirthDraftRefusal> {
        self.change("creche.suggest", ApplicationEventKind::Activate, "")?;
        self.replace_name = true;
        Ok(())
    }
    fn cycle_tradition(&mut self, backward: bool) -> Result<(), BirthDraftRefusal> {
        let count = self.draft.naming_systems().count();
        let current = self
            .draft
            .naming_systems()
            .position(|(id, _)| id == self.draft.requested_system())
            .unwrap_or(0);
        let next = (current + if backward { count - 1 } else { 1 }) % count;
        let system: String = self
            .draft
            .naming_systems()
            .nth(next)
            .expect("bounded naming index")
            .0
            .into();
        self.change("creche.naming", ApplicationEventKind::Change, &system)?;
        self.replace_name = true;
        Ok(())
    }

    fn change(
        &mut self,
        action: &str,
        event: ApplicationEventKind,
        value: &str,
    ) -> Result<(), BirthDraftRefusal> {
        match self
            .draft
            .apply_event(self.draft.revision(), action, event, value)?
        {
            BirthActionOutcome::Changed => Ok(()),
            BirthActionOutcome::Birth(_) => Err(BirthDraftRefusal::UnknownAction),
        }
    }
}

#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
fn refusal_text(error: BirthDraftRefusal) -> &'static str {
    match error {
        BirthDraftRefusal::InvalidName => "Give your body a name of at most 64 bytes.",
        BirthDraftRefusal::EmptySelection => "Include at least one plot to begin.",
        BirthDraftRefusal::UnavailablePlot => "This plot cannot run with the current capabilities.",
        BirthDraftRefusal::StaleFace => "This choice has changed. Please choose again.",
        _ => "The Crèche could not accept this change.",
    }
}

#[cfg(test)]
mod tests;

impl Arrival {
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    fn focus(&self) -> usize {
        self.focus
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    fn accept(&mut self, event: KeyEvent) -> ArrivalInput {
        use conduitos_creche_core::native::NativeOutcome;
        match self.draft.accept_native(event) {
            Ok(NativeOutcome::Unchanged) => ArrivalInput::Unchanged,
            Ok(NativeOutcome::Changed) => ArrivalInput::Changed,
            Ok(NativeOutcome::Birth(selection)) => ArrivalInput::Birth(selection),
            Err(_) => ArrivalInput::Changed,
        }
    }
}
