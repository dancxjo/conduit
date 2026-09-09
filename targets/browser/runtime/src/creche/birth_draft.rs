//! Browser transport for the same Crèche draft and actions used on native Hosts.
//! This ephemeral draft does not own Body, admission, planning, or execution.

use std::cell::RefCell;

use conduit_body::ResidentForm;
use conduit_creche_model::birth::{BirthActionOutcome, BirthDraft, BirthFormChoice};
use conduit_presentation::ApplicationEventKind;
use serde::{Deserialize, Serialize};

use super::abi;

const ERROR_DRAFT: i32 = -461;

#[cfg(test)]
mod tests;

#[derive(Default)]
struct CurrentDraft {
    generation: u32,
    draft: Option<BirthDraft>,
}
thread_local! {
    static CURRENT: RefCell<CurrentDraft> = RefCell::new(CurrentDraft::default());
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenRequest {
    persona_uuid: String,
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choice {
    title: String,
    search_text: String,
    form: ResidentForm,
    selected: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRequest {
    generation: u32,
    revision: u32,
    action: String,
    event: String,
    value: String,
}

#[derive(Serialize)]
struct Snapshot<'a> {
    generation: u32,
    revision: u32,
    friendly_name: &'a str,
    naming_system: &'a str,
    selected: Vec<&'a ResidentForm>,
    birth_requested: bool,
}

impl CurrentDraft {
    fn snapshot(&self, birth_requested: bool) -> Result<(), String> {
        let draft = self.draft.as_ref().ok_or("Crèche draft is absent")?;
        abi::write_output(&Snapshot {
            generation: self.generation,
            revision: draft.revision(),
            friendly_name: draft.friendly_name(),
            naming_system: draft.requested_system(),
            selected: draft
                .choices()
                .iter()
                .filter(|choice| choice.selected)
                .map(|choice| &choice.form)
                .collect(),
            birth_requested,
        })
        .map_err(|_| "Crèche draft snapshot exceeds its output bound".into())
    }

    fn open(&mut self, request: OpenRequest) -> Result<(), String> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or("Crèche draft generation exhausted")?;
        let choices = request
            .choices
            .into_iter()
            .map(|choice| BirthFormChoice {
                title: choice.title,
                search_text: choice.search_text,
                form: choice.form,
                selected: choice.selected,
                // This is the checked browser inventory. Combined realization is
                // still reviewed by the authoritative lifecycle boundary at Birth.
                refusal: None,
            })
            .collect();
        let draft = BirthDraft::new(request.persona_uuid, choices)
            .map_err(|error| format!("Crèche draft refused: {error:?}"))?
            .allow_idle_body();
        draft
            .presentation()
            .map_err(|error| format!("Crèche presentation refused: {error:?}"))?;
        // Admit the largest possible selection and escaped editable metadata
        // before replacing the current draft. Later events cannot outgrow the
        // response buffer after a successful state change.
        let escaped_name = "\u{001f}".repeat(64);
        let longest_system = "\u{001f}".repeat(64);
        let largest = Snapshot {
            generation: u32::MAX,
            revision: u32::MAX,
            friendly_name: &escaped_name,
            naming_system: &longest_system,
            selected: draft.choices().iter().map(|choice| &choice.form).collect(),
            birth_requested: false,
        };
        let snapshot_bytes =
            serde_json::to_vec(&largest).map_err(|_| "Crèche draft snapshot cannot be encoded")?;
        let view_bytes = draft
            .presentation()
            .map_err(|_| "Crèche view refused")?
            .lower()
            .map_err(|_| "Crèche view lowering refused")?
            .encode()
            .map_err(|_| "Crèche view encoding refused")?;
        // The unfiltered view includes every choice. Reserve the maximum name,
        // tradition and search growth, plus the empty-search result status.
        if snapshot_bytes.len() > abi::OUTPUT_BYTES
            || view_bytes.len().saturating_add(64 + 64 + 128 + 256) > abi::OUTPUT_BYTES
        {
            return Err("Crèche draft exceeds its output bound".into());
        }
        let candidate = Self {
            generation,
            draft: Some(draft),
        };
        candidate.snapshot(false)?;
        *self = candidate;
        Ok(())
    }

    fn event(&mut self, request: EventRequest) -> Result<(), String> {
        if request.generation != self.generation {
            return Err("Stale Crèche draft generation".into());
        }
        let event = match request.event.as_str() {
            "input" => ApplicationEventKind::Input,
            "change" => ApplicationEventKind::Change,
            "activate" => ApplicationEventKind::Activate,
            _ => return Err("Unknown Crèche event kind".into()),
        };
        let outcome = self
            .draft
            .as_mut()
            .ok_or("Crèche draft is absent")?
            .apply_event(request.revision, &request.action, event, &request.value)
            .map_err(|error| format!("Crèche action refused: {error:?}"))?;
        self.snapshot(matches!(outcome, BirthActionOutcome::Birth(_)))
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_birth_draft_open(length: usize) -> i32 {
    request::<OpenRequest>(length, |request| {
        CURRENT.with(|current| current.borrow_mut().open(request))
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_birth_draft_event(length: usize) -> i32 {
    request::<EventRequest>(length, |request| {
        CURRENT.with(|current| current.borrow_mut().event(request))
    })
}

#[no_mangle]
pub extern "C" fn conduit_creche_birth_draft_view(generation: u32) -> i32 {
    abi::clear_output();
    let result = CURRENT.with(|current| {
        let current = current.borrow();
        if current.generation != generation {
            return Err("Stale Crèche draft generation".into());
        }
        current
            .draft
            .as_ref()
            .ok_or("Crèche draft is absent")?
            .presentation()
            .map_err(|error| format!("Crèche presentation refused: {error:?}"))?
            .lower()
            .map_err(|error| format!("Crèche presentation lowering refused: {error:?}"))?
            .encode()
            .map_err(|error| format!("Crèche presentation encoding refused: {error:?}"))
            .and_then(|bytes| {
                abi::write_output_bytes(&bytes)
                    .map_err(|_| "Crèche presentation exceeds its output bound".into())
            })
    });
    finish(result)
}

fn request<T: serde::de::DeserializeOwned>(
    length: usize,
    action: impl FnOnce(T) -> Result<(), String>,
) -> i32 {
    abi::clear_output();
    let bytes = match abi::take_input(length) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    finish(
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("Malformed Crèche draft request: {error}"))
            .and_then(action),
    )
}

fn finish(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(message) => abi::refuse(message, ERROR_DRAFT),
    }
}
