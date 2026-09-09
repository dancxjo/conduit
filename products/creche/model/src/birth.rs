//! Renderer-neutral Crèche naming and exact initial Form selection.
use alloc::{string::String, vec::Vec};
use conduit_body::{BodyWorkset, MAX_BODY_FORMS, ResidentForm};

use crate::names::{MAX_FRIENDLY_NAME_BYTES, NameSuggestion, NamingCatalog, NamingRefusal};

mod presentation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirthFormChoice {
    pub title: String,
    pub form: ResidentForm,
    /// Current local review refusal. Availability is never inferred from a label.
    pub refusal: Option<String>,
    pub selected: bool,
}

pub struct BirthDraft {
    uuid: String,
    catalog: NamingCatalog,
    revision: u32,
    friendly_name: String,
    requested_system: String,
    suggestion: NameSuggestion,
    choices: Vec<BirthFormChoice>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirthSelection {
    pub revision: u32,
    pub friendly_name: String,
    pub workset: BodyWorkset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BirthDraftRefusal {
    Naming(NamingRefusal),
    InvalidName,
    InvalidInventory,
    EmptySelection,
    UnavailableForm,
    StalePresentation,
    RevisionExhausted,
    UnknownChoice,
}

impl BirthDraft {
    /// The caller supplies checked identities and actual local capability review.
    /// This draft never grants authority, creates a Body, or starts a Play.
    pub fn new(uuid: String, choices: Vec<BirthFormChoice>) -> Result<Self, BirthDraftRefusal> {
        if choices.is_empty()
            || choices.len() > MAX_BODY_FORMS
            || choices.iter().any(|choice| {
                choice.title.is_empty()
                    || choice.title.len() > 64
                    || choice.refusal.as_ref().is_some_and(|text| text.len() > 256)
                    || (choice.selected && choice.refusal.is_some())
            })
            || BodyWorkset::from_forms(choices.iter().map(|choice| choice.form.clone())).is_err()
        {
            return Err(BirthDraftRefusal::InvalidInventory);
        }
        let catalog = NamingCatalog::shared().map_err(BirthDraftRefusal::Naming)?;
        let suggestion = catalog
            .name_for(&uuid, "surprise", 0)
            .map_err(BirthDraftRefusal::Naming)?;
        Ok(Self {
            uuid,
            catalog,
            revision: 1,
            friendly_name: suggestion.name.clone(),
            requested_system: "surprise".into(),
            suggestion,
            choices,
        })
    }
    pub fn revision(&self) -> u32 {
        self.revision
    }
    pub fn friendly_name(&self) -> &str {
        &self.friendly_name
    }
    pub fn suggestion(&self) -> &NameSuggestion {
        &self.suggestion
    }
    pub fn choices(&self) -> &[BirthFormChoice] {
        &self.choices
    }
    pub fn naming_systems(&self) -> impl Iterator<Item = (&str, &str)> {
        core::iter::once(("surprise", "Surprise me")).chain(
            self.catalog
                .systems
                .iter()
                .map(|system| (system.id.as_str(), system.label.as_str())),
        )
    }
    pub fn requested_system(&self) -> &str {
        &self.requested_system
    }
    pub fn edit_name(&mut self, revision: u32, name: String) -> Result<(), BirthDraftRefusal> {
        let next = self.next_revision(revision)?;
        // Empty input is a valid editing state, but cannot be submitted.
        if name.len() > MAX_FRIENDLY_NAME_BYTES || name.chars().any(char::is_control) {
            return Err(BirthDraftRefusal::InvalidName);
        }
        self.friendly_name = name;
        self.revision = next;
        Ok(())
    }
    pub fn suggest(&mut self, revision: u32, system: &str) -> Result<(), BirthDraftRefusal> {
        let next = self.next_revision(revision)?;
        let variation = self
            .suggestion
            .variation
            .checked_add(1)
            .ok_or(BirthDraftRefusal::RevisionExhausted)?;
        let suggestion = self
            .catalog
            .name_for(&self.uuid, system, variation)
            .map_err(BirthDraftRefusal::Naming)?;
        self.friendly_name = suggestion.name.clone();
        self.suggestion = suggestion;
        self.requested_system = system.into();
        self.revision = next;
        Ok(())
    }
    pub fn select(
        &mut self,
        revision: u32,
        index: usize,
        selected: bool,
    ) -> Result<(), BirthDraftRefusal> {
        let next = self.next_revision(revision)?;
        let choice = self
            .choices
            .get_mut(index)
            .ok_or(BirthDraftRefusal::UnknownChoice)?;
        if selected && choice.refusal.is_some() {
            return Err(BirthDraftRefusal::UnavailableForm);
        }
        choice.selected = selected;
        self.revision = next;
        Ok(())
    }
    pub fn selection(&self, revision: u32) -> Result<BirthSelection, BirthDraftRefusal> {
        if revision != self.revision {
            return Err(BirthDraftRefusal::StalePresentation);
        }
        let name = self.friendly_name.trim();
        if name.is_empty() {
            return Err(BirthDraftRefusal::InvalidName);
        }
        let workset = BodyWorkset::from_forms(
            self.choices
                .iter()
                .filter(|choice| choice.selected)
                .map(|choice| choice.form.clone()),
        )
        .map_err(|_| BirthDraftRefusal::InvalidInventory)?;
        if workset.is_empty() {
            return Err(BirthDraftRefusal::EmptySelection);
        }
        Ok(BirthSelection {
            revision,
            friendly_name: name.into(),
            workset,
        })
    }
    fn next_revision(&self, expected: u32) -> Result<u32, BirthDraftRefusal> {
        if expected != self.revision {
            return Err(BirthDraftRefusal::StalePresentation);
        }
        self.revision
            .checked_add(1)
            .ok_or(BirthDraftRefusal::RevisionExhausted)
    }
}
