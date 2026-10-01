//! Renderer-neutral rhetoric among exact Presentation subjects.

use alloc::string::String;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::{
    identity::hash_string, presentation::validate_id, Presentation, PresentationCompositionKind,
    PresentationCompositionRelation, PresentationError,
};

pub const MAX_PRESENTATION_COMPOSITION_RELATIONS: usize = 2_048;

impl Serialize for PresentationCompositionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Group => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 0, "Group")
            }
            Self::Contrast => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 1, "Contrast")
            }
            Self::Juxtapose => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 2, "Juxtapose")
            }
            Self::Emphasize => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 3, "Emphasize")
            }
            Self::Subordinate => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 4, "Subordinate")
            }
            Self::Associate => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 5, "Associate")
            }
            Self::RevealAfter => {
                serializer.serialize_unit_variant("PresentationCompositionKind", 6, "RevealAfter")
            }
            Self::Semantic(payload) => serializer.serialize_newtype_variant(
                "PresentationCompositionKind",
                7,
                "Semantic",
                payload.identity(),
            ),
        }
    }
}

#[derive(Deserialize)]
enum PresentationCompositionKindSerde {
    Group,
    Contrast,
    Juxtapose,
    Emphasize,
    Subordinate,
    Associate,
    RevealAfter,
    Semantic(String),
}

impl<'de> Deserialize<'de> for PresentationCompositionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = PresentationCompositionKindSerde::deserialize(deserializer)?;
        match value {
            PresentationCompositionKindSerde::Group => Ok(Self::Group),
            PresentationCompositionKindSerde::Contrast => Ok(Self::Contrast),
            PresentationCompositionKindSerde::Juxtapose => Ok(Self::Juxtapose),
            PresentationCompositionKindSerde::Emphasize => Ok(Self::Emphasize),
            PresentationCompositionKindSerde::Subordinate => Ok(Self::Subordinate),
            PresentationCompositionKindSerde::Associate => Ok(Self::Associate),
            PresentationCompositionKindSerde::RevealAfter => Ok(Self::RevealAfter),
            PresentationCompositionKindSerde::Semantic(identity) => Self::semantic(identity)
                .map_err(|error| serde::de::Error::custom(alloc::format!("{error:?}"))),
        }
    }
}

impl Serialize for PresentationCompositionRelation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serializer.serialize_struct("PresentationCompositionRelation", 4)?;
        value.serialize_field("identity", &self.identity)?;
        value.serialize_field("source", &self.source)?;
        value.serialize_field("target", &self.target)?;
        value.serialize_field("kind", &self.kind)?;
        value.end()
    }
}

#[derive(Deserialize)]
struct PresentationCompositionRelationSerde {
    identity: String,
    source: String,
    target: String,
    kind: PresentationCompositionKind,
}

impl<'de> Deserialize<'de> for PresentationCompositionRelation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = PresentationCompositionRelationSerde::deserialize(deserializer)?;
        Self::new(value.identity, value.kind, value.source, value.target)
            .map_err(|error| serde::de::Error::custom(alloc::format!("{error:?}")))
    }
}

impl Presentation {
    pub(crate) fn validate_rhetorical_composition(&self) -> Result<(), PresentationError> {
        if self.composition.len() > MAX_PRESENTATION_COMPOSITION_RELATIONS {
            return Err(PresentationError::TooManyCompositionRelations);
        }
        for (index, relation) in self.composition.iter().enumerate() {
            validate_id(&relation.identity)?;
            validate_id(&relation.source)?;
            validate_id(&relation.target)?;
            if !self.has_subject(&relation.source) || !self.has_subject(&relation.target) {
                return Err(PresentationError::UnknownCompositionSubject);
            }
            if relation.source == relation.target {
                return Err(PresentationError::InvalidCompositionRelation);
            }
            if let PresentationCompositionKind::Semantic(payload) = &relation.kind {
                validate_id(payload.identity())?;
                if !payload.identity().contains('/') {
                    return Err(PresentationError::InvalidSemanticIdentity);
                }
            }
            if self.composition[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == relation.identity)
            {
                return Err(PresentationError::DuplicateCompositionRelation);
            }
        }
        Ok(())
    }

    pub(crate) fn hash_rhetorical_composition(&self, digest: &mut Sha256) {
        for relation in &self.composition {
            hash_string(digest, &relation.identity);
            hash_string(digest, &relation.source);
            hash_string(digest, &relation.target);
            match &relation.kind {
                PresentationCompositionKind::Group => digest.update([0]),
                PresentationCompositionKind::Contrast => digest.update([1]),
                PresentationCompositionKind::Juxtapose => digest.update([2]),
                PresentationCompositionKind::Emphasize => digest.update([3]),
                PresentationCompositionKind::Subordinate => digest.update([4]),
                PresentationCompositionKind::Associate => digest.update([5]),
                PresentationCompositionKind::RevealAfter => digest.update([6]),
                PresentationCompositionKind::Semantic(payload) => {
                    digest.update([u8::MAX]);
                    hash_string(digest, payload.identity());
                }
            }
        }
    }

    pub(crate) fn rhetorical_composition_len(&self) -> usize {
        self.composition
            .iter()
            .map(|relation| {
                relation.identity.len()
                    + relation.source.len()
                    + relation.target.len()
                    + match &relation.kind {
                        PresentationCompositionKind::Semantic(payload) => payload.identity().len(),
                        _ => 1,
                    }
            })
            .sum()
    }
}

pub(crate) fn linear_composition(relation: &PresentationCompositionRelation) -> String {
    alloc::format!(
        "COMPOSITION id={:?} kind={:?} source={:?} target={:?}",
        relation.identity,
        relation.kind,
        relation.source,
        relation.target
    )
}
