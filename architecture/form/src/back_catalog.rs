use crate::prelude::*;
use crate::{CheckedCanonicalForm, CheckedSyntaxDocument};
use alloc::collections::BTreeMap;
use conduit_core::{CheckedFormId, FormBack, Kind, KindId, SourceDocumentId};

pub const MAXIMUM_CANONICAL_BACKS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBackDefinition {
    pub realization: FormBack,
    pub form: CheckedCanonicalForm,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalBackCatalog {
    backs: BTreeMap<KindId, CanonicalBackDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalBackError {
    LimitExceeded,
    MissingForm(String),
    DuplicateKind(String),
    FaceMismatch(String),
    StaleSourceDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    StaleCheckedForm {
        expected: CheckedFormId,
        actual: CheckedFormId,
    },
}

impl CanonicalBackCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        kind: &Kind,
        document: &CheckedSyntaxDocument,
        form_name: &str,
    ) -> Result<(), CanonicalBackError> {
        self.insert_checked(kind, document, form_name)
    }

    /// Installs a Back only when the whole checked front is equal, including
    /// exact startup parameter names, types, and default presence.
    pub fn insert_with_startup(
        &mut self,
        kind: &Kind,
        startup: &[conduit_core::FrontStartupParameter],
        document: &CheckedSyntaxDocument,
        form_name: &str,
    ) -> Result<(), CanonicalBackError> {
        if startup != kind.startup_parameters {
            return Err(CanonicalBackError::FaceMismatch(
                kind.kind_id.as_str().into(),
            ));
        }
        self.insert_checked(kind, document, form_name)
    }

    /// Installs a reviewed Back only when the caller's admitted content
    /// identities still name these exact checked bytes. A catalog refresh may
    /// not silently retarget an existing realization choice to a newer Form.
    pub fn insert_exact(
        &mut self,
        kind: &Kind,
        startup: &[conduit_core::FrontStartupParameter],
        document: &CheckedSyntaxDocument,
        form_name: &str,
        expected_source_document_id: &SourceDocumentId,
        expected_checked_form_id: &CheckedFormId,
    ) -> Result<(), CanonicalBackError> {
        if &document.source_document_id != expected_source_document_id {
            return Err(CanonicalBackError::StaleSourceDocument {
                expected: expected_source_document_id.clone(),
                actual: document.source_document_id.clone(),
            });
        }
        let form = document
            .forms
            .iter()
            .find(|form| form.name == form_name)
            .ok_or_else(|| CanonicalBackError::MissingForm(form_name.into()))?;
        if &form.checked_form_id != expected_checked_form_id {
            return Err(CanonicalBackError::StaleCheckedForm {
                expected: expected_checked_form_id.clone(),
                actual: form.checked_form_id.clone(),
            });
        }
        if startup != kind.startup_parameters {
            return Err(CanonicalBackError::FaceMismatch(
                kind.kind_id.as_str().into(),
            ));
        }
        self.insert_checked(kind, document, form_name)
    }

    fn insert_checked(
        &mut self,
        kind: &Kind,
        document: &CheckedSyntaxDocument,
        form_name: &str,
    ) -> Result<(), CanonicalBackError> {
        if self.backs.len() >= MAXIMUM_CANONICAL_BACKS {
            return Err(CanonicalBackError::LimitExceeded);
        }
        let form = document
            .forms
            .iter()
            .find(|form| form.name == form_name)
            .cloned()
            .ok_or_else(|| CanonicalBackError::MissingForm(form_name.into()))?;
        if kind.validate().is_err() || form.checked_front() != kind.checked_front() {
            return Err(CanonicalBackError::FaceMismatch(
                kind.kind_id.as_str().into(),
            ));
        }
        let realization = FormBack {
            invocation_path: String::new(),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            source_document_id: document.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
        };
        if self
            .backs
            .insert(
                kind.kind_id.clone(),
                CanonicalBackDefinition { realization, form },
            )
            .is_some()
        {
            return Err(CanonicalBackError::DuplicateKind(
                kind.kind_id.as_str().into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn get(&self, kind: &KindId) -> Option<&CanonicalBackDefinition> {
        self.backs.get(kind)
    }
}
