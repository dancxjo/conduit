//! Renderer-neutral rhetoric among exact Presentation subjects.

use alloc::string::String;
use conduit_core::KindId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{identity::hash_string, presentation::validate_id, Presentation, PresentationError};

pub const MAX_PRESENTATION_COMPOSITION_RELATIONS: usize = 2_048;

/// How two semantic subjects are meant to stand together for a human.
///
/// This is rhetoric, not domain truth and not a physical layout instruction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PresentationCompositionKind {
    /// Read `source` as a member of the rhetorical whole named by `target`.
    Group,
    /// Understand `source` by its material difference from `target`.
    Contrast,
    /// Consider `source` and `target` together without asserting contrast.
    Juxtapose,
    /// Give `source` greater rhetorical importance than `target`.
    Emphasize,
    /// Treat `source` as supporting or qualifying `target`.
    Subordinate,
    /// Invite the human to consider `source` in connection with `target`
    /// without asserting that connection as domain truth.
    Associate,
    /// Withhold `source` until after `target` in the encounter's semantic
    /// progression; physical timing remains Mask realization.
    RevealAfter,
    /// An open, renderer-neutral rhetorical relation with the same
    /// `source`-relative-to-`target` direction.
    Semantic(KindId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationCompositionRelation {
    pub identity: String,
    pub source: String,
    pub target: String,
    pub kind: PresentationCompositionKind,
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
            if let PresentationCompositionKind::Semantic(identity) = &relation.kind {
                validate_id(identity.as_str())?;
                if !identity.as_str().contains('/') {
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
                PresentationCompositionKind::Semantic(identity) => {
                    digest.update([u8::MAX]);
                    hash_string(digest, identity.as_str());
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
                        PresentationCompositionKind::Semantic(identity) => identity.as_str().len(),
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
