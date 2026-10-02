use alloc::{string::String, vec::Vec};
use conduit_form::rust_binding::BoundedSequence;

use crate::{
    ClassificationLabel, ClassificationLabels, ExtractedField, ExtractionFields, ExtractionKey,
    ExtractionValue, FiniteClassification, StructuredResultInvalidity, ValidatedExtraction,
};

pub const MAXIMUM_CLASSIFICATION_LABELS: usize = 32;
pub const MAXIMUM_CLASSIFICATION_LABEL_BYTES: usize = 64;
pub const MAXIMUM_EXTRACTION_FIELDS: usize = 32;
pub const MAXIMUM_EXTRACTION_KEY_BYTES: usize = 64;
pub const MAXIMUM_EXTRACTION_VALUE_BYTES: usize = 1_024;
pub const MAXIMUM_EMBEDDING_PROFILE_BYTES: usize = 128;
pub const MAXIMUM_EMBEDDING_DIMENSIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FiniteEmbedding {
    pub profile_identity: String,
    pub dimensions: u32,
    pub values: Vec<f32>,
}

impl FiniteClassification {
    pub fn from_strings(
        label: String,
        allowed_labels: Vec<String>,
    ) -> Result<Self, StructuredResultInvalidity> {
        if label.is_empty() || allowed_labels.is_empty() {
            return Err(StructuredResultInvalidity::Empty);
        }
        if allowed_labels.len() > MAXIMUM_CLASSIFICATION_LABELS {
            return Err(StructuredResultInvalidity::TooManyMembers);
        }
        if label.len() > MAXIMUM_CLASSIFICATION_LABEL_BYTES
            || allowed_labels.iter().any(|candidate| {
                candidate.is_empty() || candidate.len() > MAXIMUM_CLASSIFICATION_LABEL_BYTES
            })
        {
            return Err(StructuredResultInvalidity::MemberTooLarge);
        }
        let label = ClassificationLabel::new(label)
            .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
        let allowed_labels = allowed_labels
            .into_iter()
            .map(ClassificationLabel::new)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
        let allowed_labels = BoundedSequence::try_from_iter(allowed_labels)
            .map_err(|_| StructuredResultInvalidity::TooManyMembers)?;
        let candidate = Self::new(
            label,
            ClassificationLabels::new(allowed_labels)
                .map_err(|_| StructuredResultInvalidity::TooManyMembers)?,
        )
        .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> Result<(), StructuredResultInvalidity> {
        if has_duplicates(self.allowed_labels.get().as_slice()) {
            return Err(StructuredResultInvalidity::DuplicateMember);
        }
        if !self.allowed_labels.get().contains(&self.label) {
            return Err(StructuredResultInvalidity::LabelNotAllowed);
        }
        Ok(())
    }
}

impl ValidatedExtraction {
    pub fn from_strings(
        schema_identity: String,
        fields: Vec<(String, String)>,
    ) -> Result<Self, StructuredResultInvalidity> {
        if schema_identity.is_empty() || fields.is_empty() {
            return Err(StructuredResultInvalidity::Empty);
        }
        if fields.len() > MAXIMUM_EXTRACTION_FIELDS {
            return Err(StructuredResultInvalidity::TooManyMembers);
        }
        if fields.iter().any(|(key, value)| {
            key.is_empty()
                || value.is_empty()
                || key.len() > MAXIMUM_EXTRACTION_KEY_BYTES
                || value.len() > MAXIMUM_EXTRACTION_VALUE_BYTES
        }) {
            return Err(StructuredResultInvalidity::MemberTooLarge);
        }
        let fields = fields
            .into_iter()
            .map(|(key, value)| {
                ExtractedField::new(
                    ExtractionKey::new(key)
                        .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?,
                    ExtractionValue::new(value)
                        .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?,
                )
                .map_err(|_| StructuredResultInvalidity::MemberTooLarge)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fields = BoundedSequence::try_from_iter(fields)
            .map_err(|_| StructuredResultInvalidity::TooManyMembers)?;
        let candidate = Self::new(
            schema_identity,
            ExtractionFields::new(fields)
                .map_err(|_| StructuredResultInvalidity::TooManyMembers)?,
        )
        .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> Result<(), StructuredResultInvalidity> {
        if self.fields.get().iter().enumerate().any(|(index, field)| {
            self.fields.get().as_slice()[index + 1..]
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

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values.iter().enumerate().any(|(index, value)| {
        values[index + 1..]
            .iter()
            .any(|candidate| candidate == value)
    })
}
