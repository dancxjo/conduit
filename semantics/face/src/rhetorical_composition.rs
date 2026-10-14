//! Renderer-neutral rhetoric among exact Face subjects.

use alloc::string::String;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::{
    face::validate_id, identity::hash_string, Face, FaceCompositionKind, FaceCompositionRelation,
    FaceError,
};

pub const MAX_FACE_COMPOSITION_RELATIONS: usize = 2_048;

impl Serialize for FaceCompositionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Group => serializer.serialize_unit_variant("FaceCompositionKind", 0, "Group"),
            Self::Contrast => {
                serializer.serialize_unit_variant("FaceCompositionKind", 1, "Contrast")
            }
            Self::Juxtapose => {
                serializer.serialize_unit_variant("FaceCompositionKind", 2, "Juxtapose")
            }
            Self::Emphasize => {
                serializer.serialize_unit_variant("FaceCompositionKind", 3, "Emphasize")
            }
            Self::Subordinate => {
                serializer.serialize_unit_variant("FaceCompositionKind", 4, "Subordinate")
            }
            Self::Associate => {
                serializer.serialize_unit_variant("FaceCompositionKind", 5, "Associate")
            }
            Self::RevealAfter => {
                serializer.serialize_unit_variant("FaceCompositionKind", 6, "RevealAfter")
            }
            Self::Semantic(payload) => serializer.serialize_newtype_variant(
                "FaceCompositionKind",
                7,
                "Semantic",
                payload.identity(),
            ),
        }
    }
}

#[derive(Deserialize)]
enum FaceCompositionKindSerde {
    Group,
    Contrast,
    Juxtapose,
    Emphasize,
    Subordinate,
    Associate,
    RevealAfter,
    Semantic(String),
}

impl<'de> Deserialize<'de> for FaceCompositionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = FaceCompositionKindSerde::deserialize(deserializer)?;
        match value {
            FaceCompositionKindSerde::Group => Ok(Self::Group),
            FaceCompositionKindSerde::Contrast => Ok(Self::Contrast),
            FaceCompositionKindSerde::Juxtapose => Ok(Self::Juxtapose),
            FaceCompositionKindSerde::Emphasize => Ok(Self::Emphasize),
            FaceCompositionKindSerde::Subordinate => Ok(Self::Subordinate),
            FaceCompositionKindSerde::Associate => Ok(Self::Associate),
            FaceCompositionKindSerde::RevealAfter => Ok(Self::RevealAfter),
            FaceCompositionKindSerde::Semantic(identity) => Self::semantic(identity)
                .map_err(|error| serde::de::Error::custom(alloc::format!("{error:?}"))),
        }
    }
}

impl Serialize for FaceCompositionRelation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serializer.serialize_struct("FaceCompositionRelation", 4)?;
        value.serialize_field("identity", &self.identity)?;
        value.serialize_field("source", &self.source)?;
        value.serialize_field("target", &self.target)?;
        value.serialize_field("kind", &self.kind)?;
        value.end()
    }
}

#[derive(Deserialize)]
struct FaceCompositionRelationSerde {
    identity: String,
    source: String,
    target: String,
    kind: FaceCompositionKind,
}

impl<'de> Deserialize<'de> for FaceCompositionRelation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = FaceCompositionRelationSerde::deserialize(deserializer)?;
        Self::new(value.identity, value.kind, value.source, value.target)
            .map_err(|error| serde::de::Error::custom(alloc::format!("{error:?}")))
    }
}

impl Face {
    pub(crate) fn validate_rhetorical_composition(&self) -> Result<(), FaceError> {
        if self.composition.len() > MAX_FACE_COMPOSITION_RELATIONS {
            return Err(FaceError::TooManyCompositionRelations);
        }
        for (index, relation) in self.composition.iter().enumerate() {
            validate_id(&relation.identity)?;
            validate_id(&relation.source)?;
            validate_id(&relation.target)?;
            if !self.has_subject(&relation.source) || !self.has_subject(&relation.target) {
                return Err(FaceError::UnknownCompositionSubject);
            }
            if relation.source == relation.target {
                return Err(FaceError::InvalidCompositionRelation);
            }
            if let FaceCompositionKind::Semantic(payload) = &relation.kind {
                validate_id(payload.identity())?;
                if !payload.identity().contains('/') {
                    return Err(FaceError::InvalidSemanticIdentity);
                }
            }
            if self.composition[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == relation.identity)
            {
                return Err(FaceError::DuplicateCompositionRelation);
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
                FaceCompositionKind::Group => digest.update([0]),
                FaceCompositionKind::Contrast => digest.update([1]),
                FaceCompositionKind::Juxtapose => digest.update([2]),
                FaceCompositionKind::Emphasize => digest.update([3]),
                FaceCompositionKind::Subordinate => digest.update([4]),
                FaceCompositionKind::Associate => digest.update([5]),
                FaceCompositionKind::RevealAfter => digest.update([6]),
                FaceCompositionKind::Semantic(payload) => {
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
                        FaceCompositionKind::Semantic(payload) => payload.identity().len(),
                        _ => 1,
                    }
            })
            .sum()
    }
}

pub(crate) fn linear_composition(relation: &FaceCompositionRelation) -> String {
    alloc::format!(
        "COMPOSITION id={:?} kind={:?} source={:?} target={:?}",
        relation.identity,
        relation.kind,
        relation.source,
        relation.target
    )
}
