use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_CLASSIFICATION_LABELS: usize = 32;
pub const MAXIMUM_CLASSIFICATION_LABEL_BYTES: usize = 64;
pub const MAXIMUM_EXTRACTION_FIELDS: usize = 32;
pub const MAXIMUM_EXTRACTION_KEY_BYTES: usize = 64;
pub const MAXIMUM_EXTRACTION_VALUE_BYTES: usize = 1_024;
pub const MAXIMUM_EMBEDDING_PROFILE_BYTES: usize = 128;
pub const MAXIMUM_EMBEDDING_DIMENSIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiniteClassification {
    pub label: String,
    pub allowed_labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedExtraction {
    pub schema_identity: String,
    pub fields: Vec<ExtractedField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiniteEmbedding {
    pub profile_identity: String,
    pub dimensions: u32,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StructuredResultInvalidity {
    Empty,
    TooManyMembers,
    MemberTooLarge,
    DuplicateMember,
    LabelNotAllowed,
    DimensionMismatch,
    NonFiniteValue,
}

impl FiniteClassification {
    pub fn validate(&self) -> Result<(), StructuredResultInvalidity> {
        if self.label.is_empty() || self.allowed_labels.is_empty() {
            return Err(StructuredResultInvalidity::Empty);
        }
        if self.allowed_labels.len() > MAXIMUM_CLASSIFICATION_LABELS {
            return Err(StructuredResultInvalidity::TooManyMembers);
        }
        if self.label.len() > MAXIMUM_CLASSIFICATION_LABEL_BYTES
            || self
                .allowed_labels
                .iter()
                .any(|label| label.is_empty() || label.len() > MAXIMUM_CLASSIFICATION_LABEL_BYTES)
        {
            return Err(StructuredResultInvalidity::MemberTooLarge);
        }
        if has_duplicates(&self.allowed_labels) {
            return Err(StructuredResultInvalidity::DuplicateMember);
        }
        if !self.allowed_labels.contains(&self.label) {
            return Err(StructuredResultInvalidity::LabelNotAllowed);
        }
        Ok(())
    }
}

impl ValidatedExtraction {
    pub fn validate(&self) -> Result<(), StructuredResultInvalidity> {
        if self.schema_identity.is_empty() || self.fields.is_empty() {
            return Err(StructuredResultInvalidity::Empty);
        }
        if self.fields.len() > MAXIMUM_EXTRACTION_FIELDS {
            return Err(StructuredResultInvalidity::TooManyMembers);
        }
        if self.fields.iter().any(|field| {
            field.key.is_empty()
                || field.value.is_empty()
                || field.key.len() > MAXIMUM_EXTRACTION_KEY_BYTES
                || field.value.len() > MAXIMUM_EXTRACTION_VALUE_BYTES
        }) {
            return Err(StructuredResultInvalidity::MemberTooLarge);
        }
        if self.fields.iter().enumerate().any(|(index, field)| {
            self.fields[index + 1..]
                .iter()
                .any(|candidate| candidate.key == field.key)
        }) {
            return Err(StructuredResultInvalidity::DuplicateMember);
        }
        Ok(())
    }
}

impl FiniteEmbedding {
    pub fn validate(&self) -> Result<(), StructuredResultInvalidity> {
        if self.profile_identity.is_empty() || self.values.is_empty() {
            return Err(StructuredResultInvalidity::Empty);
        }
        if self.profile_identity.len() > MAXIMUM_EMBEDDING_PROFILE_BYTES
            || self.values.len() > MAXIMUM_EMBEDDING_DIMENSIONS
        {
            return Err(StructuredResultInvalidity::MemberTooLarge);
        }
        if self.dimensions as usize != self.values.len() {
            return Err(StructuredResultInvalidity::DimensionMismatch);
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(StructuredResultInvalidity::NonFiniteValue);
        }
        Ok(())
    }
}

fn has_duplicates(values: &[String]) -> bool {
    values.iter().enumerate().any(|(index, value)| {
        values[index + 1..]
            .iter()
            .any(|candidate| candidate == value)
    })
}
