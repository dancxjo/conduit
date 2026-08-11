use crate::prelude::*;
use crate::{CheckedCanonicalForm, CheckedSyntaxDocument, KindDefinition};
use alloc::collections::BTreeMap;
use conduit_core::{CheckedFace, KindId, RealizationBack};

pub const MAXIMUM_CANONICAL_BACKS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBackDefinition {
    pub realization: RealizationBack,
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
}

impl CanonicalBackCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        kind: &KindDefinition,
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
        if form.checked_face() != definition_face(kind) {
            return Err(CanonicalBackError::FaceMismatch(
                kind.kind_id.as_str().into(),
            ));
        }
        let realization = RealizationBack {
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

fn definition_face(kind: &KindDefinition) -> CheckedFace {
    CheckedFace::new(
        Vec::new(),
        kind.inputs.clone(),
        kind.outputs.clone(),
        match (kind.inputs.as_slice(), kind.outputs.as_slice()) {
            ([input], [output]) => Some((input.port_id.clone(), output.port_id.clone())),
            _ => None,
        },
    )
}
