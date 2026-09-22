//! Checked meaning of the exact forms offered at native birth.
use super::WorksetRefusal;
use conduit_body::ResidentForm;
use conduit_form::{ExpandedCanonicalForm, ProfileCatalog, StartupCatalog};

/// Finite native product profile. Capacity is a reviewed deployment choice,
/// not a claim that a body conceptually consists of these particular Forms.
pub const NATIVE_FORM_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFormProfile {
    pub id: &'static str,
    pub capacity: usize,
    installed: [NativeForm; NATIVE_FORM_CAPACITY],
}

impl NativeFormProfile {
    pub const fn installed(&self) -> &[NativeForm] {
        &self.installed
    }

    pub fn contains(&self, form: &ResidentForm) -> Result<bool, WorksetRefusal> {
        for installed in self.installed {
            if &resident(installed)? == form {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeForm {
    KeyboardCanvas,
    MemoryLantern,
    Tour,
    Patchbay,
}

impl NativeForm {
    pub const fn name(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "conduitos-keyboard-upper",
            Self::MemoryLantern => "memory_lantern",
            Self::Tour => "tour",
            Self::Patchbay => "patchbay",
        }
    }
    pub const fn title(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => "Keyboard canvas",
            Self::MemoryLantern => "Memory Lantern",
            Self::Tour => "Tour",
            Self::Patchbay => "Patchbay",
        }
    }
    pub const fn source(self) -> &'static str {
        match self {
            Self::KeyboardCanvas => crate::keyboard_text_plan::FORM_SOURCE,
            Self::MemoryLantern => include_str!("../../../../forms/memory-lantern/main.conduit"),
            Self::Tour => include_str!("../../../../forms/tour/main.conduit"),
            Self::Patchbay => include_str!("../../../../forms/patchbay/main.conduit"),
        }
    }
}

pub const fn profile() -> NativeFormProfile {
    NativeFormProfile {
        id: "conduitos/native-installed-forms@1",
        capacity: NATIVE_FORM_CAPACITY,
        installed: [
            NativeForm::KeyboardCanvas,
            NativeForm::MemoryLantern,
            NativeForm::Tour,
            NativeForm::Patchbay,
        ],
    }
}

pub const fn inventory() -> [NativeForm; NATIVE_FORM_CAPACITY] {
    profile().installed
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
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile)
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

pub fn resolve(identity: &ResidentForm) -> Result<NativeForm, WorksetRefusal> {
    for form in profile().installed {
        if &resident(form)? == identity {
            return Ok(form);
        }
    }
    Err(WorksetRefusal::UnknownForm)
}
