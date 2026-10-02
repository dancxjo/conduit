use alloc::{string::String, vec::Vec};
use conduit_form::rust_binding::BoundedSequence;

use crate::{
    ClassificationLabel, ClassificationLabels, EmbeddingProfileIdentity, ExtractedField,
    ExtractionFields, ExtractionKey, ExtractionValue, FiniteClassification, FiniteEmbedding,
    FiniteEmbeddingValuePage, FiniteEmbeddingValuePages, FiniteF32, StructuredResultInvalidity,
    ValidatedExtraction,
};

pub const MAXIMUM_CLASSIFICATION_LABELS: usize = 32;
pub const MAXIMUM_CLASSIFICATION_LABEL_BYTES: usize = 64;
pub const MAXIMUM_EXTRACTION_FIELDS: usize = 32;
pub const MAXIMUM_EXTRACTION_KEY_BYTES: usize = 64;
pub const MAXIMUM_EXTRACTION_VALUE_BYTES: usize = 1_024;
pub const MAXIMUM_EMBEDDING_PROFILE_BYTES: usize = 128;
pub const MAXIMUM_EMBEDDING_DIMENSIONS: usize = 4_096;

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
    pub fn values_f32(&self) -> Vec<f32> {
        self.values()
            .get()
            .iter()
            .flat_map(|page| page.get().iter())
            .map(|value| value.get().value())
            .collect()
    }

    pub fn from_values(
        profile_identity: String,
        values: Vec<f32>,
    ) -> Result<Self, StructuredResultInvalidity> {
        if profile_identity.is_empty() || values.is_empty() {
            return Err(StructuredResultInvalidity::Empty);
        }
        if profile_identity.len() > MAXIMUM_EMBEDDING_PROFILE_BYTES
            || values.len() > MAXIMUM_EMBEDDING_DIMENSIONS
        {
            return Err(StructuredResultInvalidity::MemberTooLarge);
        }
        let dimensions =
            u32::try_from(values.len()).map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
        let values = values
            .into_iter()
            .map(|value| {
                FiniteF32::new(conduit_core::IeeeF32::from(value))
                    .map_err(|_| StructuredResultInvalidity::NonFiniteValue)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pages = values
            .chunks(1_024)
            .map(|page| {
                let page = BoundedSequence::try_from_iter(page.iter().copied())
                    .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
                FiniteEmbeddingValuePage::new(page)
                    .map_err(|_| StructuredResultInvalidity::MemberTooLarge)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pages = BoundedSequence::try_from_iter(pages)
            .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?;
        Self::new(
            dimensions,
            EmbeddingProfileIdentity::new(profile_identity)
                .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?,
            FiniteEmbeddingValuePages::new(pages)
                .map_err(|_| StructuredResultInvalidity::MemberTooLarge)?,
        )
        .map_err(|_| StructuredResultInvalidity::MemberTooLarge)
    }

    pub fn validate(&self) -> Result<(), StructuredResultInvalidity> {
        let value_count = self
            .values()
            .get()
            .iter()
            .map(|page| page.get().len())
            .sum::<usize>();
        if *self.dimensions() as usize != value_count {
            return Err(StructuredResultInvalidity::DimensionMismatch);
        }
        Ok(())
    }
}

impl serde::Serialize for FiniteEmbedding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let values = self
            .values()
            .get()
            .iter()
            .flat_map(|page| page.get().iter())
            .map(|value| value.get().value())
            .collect::<Vec<_>>();
        let mut state = serializer.serialize_struct("FiniteEmbedding", 3)?;
        state.serialize_field("profile_identity", self.profile_identity().get())?;
        state.serialize_field("dimensions", self.dimensions())?;
        state.serialize_field("values", &values)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for FiniteEmbedding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct WireEmbedding {
            profile_identity: String,
            dimensions: u32,
            values: Vec<f32>,
        }

        let wire = WireEmbedding::deserialize(deserializer)?;
        let embedding = Self::from_values(wire.profile_identity, wire.values)
            .map_err(|_| serde::de::Error::custom("invalid finite embedding"))?;
        if *embedding.dimensions() != wire.dimensions {
            return Err(serde::de::Error::custom("embedding dimension mismatch"));
        }
        Ok(embedding)
    }
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values.iter().enumerate().any(|(index, value)| {
        values[index + 1..]
            .iter()
            .any(|candidate| candidate == value)
    })
}
