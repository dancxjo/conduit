//! Checked meaning of the exact Forms offered at native birth.
use super::WorksetRefusal;
use conduit_body::ResidentForm;
use conduit_form::{ExpandedCanonicalForm, ProfileCatalog, StartupCatalog};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeForm {
    KeyboardCanvas,
    MemoryLantern,
}

impl NativeForm {
    pub const fn name(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "conduitos-keyboard-upper",
            Self::MemoryLantern => "memory_lantern",
        }
    }
    pub const fn title(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "Keyboard canvas",
            Self::MemoryLantern => "Memory Lantern",
        }
    }
    pub const fn source(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => crate::keyboard_text_plan::FORM_SOURCE,
            Self::MemoryLantern => include_str!("../../../../forms/memory-lantern/main.conduit"),
        }
    }
}

pub const fn inventory() -> [NativeForm; 2] {
    [NativeForm::KeyboardCanvas, NativeForm::MemoryLantern]
}

pub fn checked(form: NativeForm) -> Result<ExpandedCanonicalForm, WorksetRefusal> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile)
        .map_err(|_| WorksetRefusal::Catalog)?;
    let syntax = conduit_form::parse_syntax_document(form.source());
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| WorksetRefusal::Catalog)?;
    conduit_form::expand_canonical_form(&checked, form.name(), &profile)
        .map_err(|_| WorksetRefusal::Catalog)
}

pub fn resident(form: NativeForm) -> Result<ResidentForm, WorksetRefusal> {
    let checked = checked(form)?;
    Ok(ResidentForm::new(
        checked.source_document_id,
        checked.checked_form_id,
    ))
}

pub(super) fn resolve(identity: &ResidentForm) -> Result<NativeForm, WorksetRefusal> {
    for form in inventory() {
        if &resident(form)? == identity {
            return Ok(form);
        }
    }
    Err(WorksetRefusal::UnknownForm)
}
