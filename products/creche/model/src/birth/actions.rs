//! One action boundary for the browser and native Crèche Presenters.

use super::{BirthDraft, BirthDraftRefusal, BirthSelection};
use alloc::{format, string::String};
use conduit_presentation::ApplicationEventKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BirthActionOutcome {
    Changed,
    /// A selection for the owning lifecycle boundary, not a born Body.
    Birth(BirthSelection),
}

impl BirthDraft {
    pub fn apply_event(
        &mut self,
        revision: u32,
        action: &str,
        event: ApplicationEventKind,
        value: &str,
    ) -> Result<BirthActionOutcome, BirthDraftRefusal> {
        if revision != self.revision() {
            return Err(BirthDraftRefusal::StalePresentation);
        }
        if action.len() > 64 || value.len() > 128 {
            return Err(BirthDraftRefusal::InvalidActionValue);
        }
        match (action, event) {
            ("creche.name", ApplicationEventKind::Input) => {
                if value.len() > crate::names::MAX_FRIENDLY_NAME_BYTES {
                    return Err(BirthDraftRefusal::InvalidName);
                }
                self.edit_name(revision, value.into())?;
            }
            ("creche.search", ApplicationEventKind::Input) if self.choices().len() > 1 => {
                self.search_forms(revision, value)?;
            }
            ("creche.suggest", ApplicationEventKind::Activate) if value.is_empty() => {
                let system: String = self.requested_system().into();
                self.suggest(revision, &system)?;
            }
            ("creche.birth", ApplicationEventKind::Activate) if value.is_empty() => {
                return self.selection(revision).map(BirthActionOutcome::Birth);
            }
            (_, ApplicationEventKind::Change) if action.starts_with("creche.form.") => {
                let index = exact_index(action, "creche.form.")?;
                self.select(revision, index, boolean(value)?)?;
            }
            ("creche.naming", ApplicationEventKind::Change) => {
                let system: String = self
                    .naming_systems()
                    .find(|(id, _)| *id == value)
                    .ok_or(BirthDraftRefusal::UnknownChoice)?
                    .0
                    .into();
                self.suggest(revision, &system)?;
            }
            _ => return Err(BirthDraftRefusal::UnknownAction),
        }
        Ok(BirthActionOutcome::Changed)
    }
}

fn exact_index(action: &str, prefix: &str) -> Result<usize, BirthDraftRefusal> {
    let index = action
        .strip_prefix(prefix)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|index| action == format!("{prefix}{index}"))
        .ok_or(BirthDraftRefusal::UnknownAction)?;
    Ok(index)
}

fn boolean(value: &str) -> Result<bool, BirthDraftRefusal> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(BirthDraftRefusal::InvalidActionValue),
    }
}
