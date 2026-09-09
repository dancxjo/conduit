//! Native input adaptation for the shared Crèche draft; no lifecycle authority.
use alloc::{format, string::String, vec::Vec};
use conduit_creche_model::birth::{
    BirthActionOutcome, BirthDraft, BirthDraftRefusal, BirthFormChoice, BirthSelection,
};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyModifiers, KeyTransition, KeymapDisposition};
use conduit_presentation::ApplicationEventKind;

use super::{Error, FrontDoor};

mod projection;
mod scene;

pub(super) struct Arrival {
    draft: BirthDraft,
    focus: usize,
    replace_name: bool,
    keymap: ConduitIntlKeymap,
    refusal: Option<String>,
}

pub enum ArrivalInput {
    Unchanged,
    Changed,
    Birth(BirthSelection),
}

impl FrontDoor {
    pub fn open_creche(&mut self, uuid: String, refusal: Option<String>) -> Result<(), Error> {
        if !self.lifecycle_authority_admitted {
            return Err(Error::ActionUnavailable);
        }
        if self.arrival.is_some() || self.journey.as_ref().is_some_and(|j| j.body_id.is_some()) {
            return Err(Error::Presentation);
        }
        let available = refusal.is_none();
        let draft = BirthDraft::new(
            uuid,
            crate::native_workset::inventory()
                .into_iter()
                .map(|form| {
                    Ok(BirthFormChoice {
                        title: form.title().into(),
                        search_text: form.source().into(),
                        form: crate::native_workset::resident(form)
                            .map_err(|_| Error::Presentation)?,
                        refusal: refusal.clone(),
                        selected: available,
                    })
                })
                .collect::<Result<_, Error>>()?,
        )
        .map_err(|_| Error::Presentation)?;
        self.arrival = Some(Arrival {
            draft,
            focus: 0,
            replace_name: true,
            keymap: ConduitIntlKeymap::new(),
            refusal: None,
        });
        self.advance()
    }

    pub fn creche_open(&self) -> bool {
        self.arrival.is_some()
    }

    pub fn close_creche(&mut self) -> Result<(), Error> {
        if self.journey.as_ref().is_none_or(|j| j.body_id.is_none()) {
            return Err(Error::Presentation);
        }
        self.arrival = None;
        self.advance()
    }

    pub fn accept_creche(&mut self, event: KeyEvent, revision: u64) -> Result<ArrivalInput, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        if event.transition() == KeyTransition::Pressed {
            self.revision.checked_add(1).ok_or(Error::Presentation)?;
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

impl Arrival {
    fn accept(&mut self, event: KeyEvent) -> ArrivalInput {
        if event.transition() != KeyTransition::Pressed {
            return ArrivalInput::Unchanged;
        }
        let controls = self.controls();
        let Some(action) = controls.get(self.focus) else {
            return ArrivalInput::Unchanged;
        };
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
            40 if action == "creche.name" || action == "creche.birth" => {
                return self.submit();
            }
            40 | 44 if self.focus == 2 => self.suggest(),
            40 | 44 if self.focus == 1 => self.cycle_tradition(false),
            40 | 44 if action.starts_with("creche.form.") => {
                let Some(index) = action
                    .strip_prefix("creche.form.")
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
                    &format!("creche.form.{index}"),
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
                KeymapDisposition::Text(fragment) => {
                    let mut search = String::from(self.draft.search());
                    let Ok(text) = core::str::from_utf8(fragment.as_bytes()) else {
                        return ArrivalInput::Unchanged;
                    };
                    search.push_str(text);
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
                KeymapDisposition::Text(fragment) => {
                    let mut name = if self.replace_name {
                        String::new()
                    } else {
                        self.draft.friendly_name().into()
                    };
                    let Ok(text) = core::str::from_utf8(fragment.as_bytes()) else {
                        return ArrivalInput::Unchanged;
                    };
                    name.push_str(text);
                    let result = self.change("creche.name", ApplicationEventKind::Input, &name);
                    if result.is_ok() {
                        self.replace_name = false;
                    }
                    result
                }
                _ => return ArrivalInput::Unchanged,
            },
            _ => return ArrivalInput::Unchanged,
        };
        self.refusal = result.err().map(|error| refusal_text(error).into());
        ArrivalInput::Changed
    }

    fn controls(&self) -> Vec<String> {
        use conduit_presentation::PresentationMechanism;
        let Ok(view) = self.draft.presentation() else {
            return Vec::new();
        };
        let mut controls = Vec::new();
        for node in &view.root.children {
            match &node.mechanism {
                PresentationMechanism::FormField(field) => {
                    controls.push(field.input_action.identity.clone())
                }
                PresentationMechanism::Action(action) => controls.push(action.identity.clone()),
                PresentationMechanism::ChoiceGroup { options, .. } => controls.extend(
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

fn refusal_text(error: BirthDraftRefusal) -> &'static str {
    match error {
        BirthDraftRefusal::InvalidName => "Give your Body a name of at most 64 bytes.",
        BirthDraftRefusal::EmptySelection => "Include at least one Form to begin.",
        BirthDraftRefusal::UnavailableForm => "This Form cannot run with the current capabilities.",
        BirthDraftRefusal::StalePresentation => "This choice has changed. Please choose again.",
        _ => "The Crèche could not accept this change.",
    }
}

#[cfg(test)]
mod tests;
