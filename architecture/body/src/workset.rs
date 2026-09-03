//! Exact bounded set of Forms currently intended by one Body.
//!
//! An entry is only the existing source/check identity pair. It deliberately
//! does not introduce a `ProgramId` or another semantic object around Form.

use alloc::vec::Vec;
use conduit_core::{CheckedFormId, SourceDocumentId};
use serde::{Deserialize, Serialize};

pub const MAX_BODY_FORMS: usize = 16;
pub const MAX_BODY_FORM_IDENTITY_BYTES: usize = 2_048;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResidentForm {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
}

impl ResidentForm {
    pub fn new(source_document_id: SourceDocumentId, checked_form_id: CheckedFormId) -> Self {
        Self {
            source_document_id,
            checked_form_id,
        }
    }

    fn identity_bytes(&self) -> Option<usize> {
        self.source_document_id
            .as_str()
            .len()
            .checked_add(self.checked_form_id.as_str().len())
    }

    fn valid(&self) -> bool {
        !self.source_document_id.as_str().is_empty()
            && !self.checked_form_id.as_str().is_empty()
            && self.identity_bytes().is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyWorkset {
    forms: Vec<ResidentForm>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyWorksetError {
    InvalidFormIdentity,
    DuplicateForm,
    FormAbsent,
    FormCapacityExhausted,
    IdentityBytesExhausted,
}

impl BodyWorkset {
    pub fn one(form: ResidentForm) -> Result<Self, BodyWorksetError> {
        let mut workset = Self::default();
        workset.add(form)?;
        Ok(workset)
    }

    pub fn from_forms(
        forms: impl IntoIterator<Item = ResidentForm>,
    ) -> Result<Self, BodyWorksetError> {
        let mut workset = Self::default();
        for form in forms {
            workset.add(form)?;
        }
        Ok(workset)
    }

    pub fn forms(&self) -> &[ResidentForm] {
        &self.forms
    }

    pub fn len(&self) -> usize {
        self.forms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.forms.is_empty()
    }

    pub fn identity_bytes(&self) -> usize {
        self.forms
            .iter()
            .map(|form| {
                form.identity_bytes()
                    .expect("validated Form identity bytes")
            })
            .sum()
    }

    pub fn contains(&self, form: &ResidentForm) -> bool {
        self.forms.binary_search(form).is_ok()
    }

    pub fn add(&mut self, form: ResidentForm) -> Result<(), BodyWorksetError> {
        self.validate()?;
        if !form.valid() {
            return Err(BodyWorksetError::InvalidFormIdentity);
        }
        let position = match self.forms.binary_search(&form) {
            Ok(_) => return Err(BodyWorksetError::DuplicateForm),
            Err(position) => position,
        };
        if self.forms.len() >= MAX_BODY_FORMS {
            return Err(BodyWorksetError::FormCapacityExhausted);
        }
        let next_bytes = self
            .identity_bytes()
            .checked_add(
                form.identity_bytes()
                    .ok_or(BodyWorksetError::IdentityBytesExhausted)?,
            )
            .ok_or(BodyWorksetError::IdentityBytesExhausted)?;
        if next_bytes > MAX_BODY_FORM_IDENTITY_BYTES {
            return Err(BodyWorksetError::IdentityBytesExhausted);
        }
        self.forms.insert(position, form);
        Ok(())
    }

    pub fn remove(&mut self, form: &ResidentForm) -> Result<(), BodyWorksetError> {
        self.validate()?;
        let position = self
            .forms
            .binary_search(form)
            .map_err(|_| BodyWorksetError::FormAbsent)?;
        self.forms.remove(position);
        Ok(())
    }

    pub fn validate(&self) -> Result<(), BodyWorksetError> {
        if self.forms.len() > MAX_BODY_FORMS {
            return Err(BodyWorksetError::FormCapacityExhausted);
        }
        let mut bytes = 0usize;
        for (index, form) in self.forms.iter().enumerate() {
            if !form.valid() {
                return Err(BodyWorksetError::InvalidFormIdentity);
            }
            if index > 0 && self.forms[index - 1] >= *form {
                return Err(BodyWorksetError::DuplicateForm);
            }
            bytes = bytes
                .checked_add(
                    form.identity_bytes()
                        .ok_or(BodyWorksetError::IdentityBytesExhausted)?,
                )
                .ok_or(BodyWorksetError::IdentityBytesExhausted)?;
        }
        if bytes > MAX_BODY_FORM_IDENTITY_BYTES {
            return Err(BodyWorksetError::IdentityBytesExhausted);
        }
        Ok(())
    }
}
