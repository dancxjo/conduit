//! Bounded temporal context without ambient clocks or face policy.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::SignId;
pub use conduit_core::{TemporalInstant, TemporalRelation, TemporalRelationError, TemporalScale};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::face::validate_id;
use crate::identity::hash_string;
use crate::{
    Face, FaceAction, FaceBasis, FaceContentId, FaceDisclosure, FaceError, FaceProperty,
    FaceRelationship, FaceSubject, FaceTemporalRole, FaceText,
};

pub const MAX_TEMPORAL_REFERENCES: usize = 256;
pub const MAX_FACE_TEMPORAL_FACTS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalReference {
    pub identity: String,
    pub instant: TemporalInstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceTemporalFact {
    pub subject: String,
    pub role: FaceTemporalRole,
    pub sign_id: Option<SignId>,
    pub source: TemporalInstant,
    pub reference: String,
    pub relation: TemporalRelation,
}

impl FaceTemporalFact {
    pub fn new(
        subject: String,
        role: FaceTemporalRole,
        sign_id: Option<SignId>,
        source: TemporalInstant,
        reference: &TemporalReference,
    ) -> Result<Self, TemporalRelationError> {
        let relation = source.relation_to(&reference.instant)?;
        Ok(Self {
            subject,
            role,
            sign_id,
            source,
            reference: reference.identity.clone(),
            relation,
        })
    }
}

impl Face {
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_semantics_and_temporal(
        revision: u64,
        mut basis: FaceBasis,
        subjects: Vec<FaceSubject>,
        relationships: Vec<FaceRelationship>,
        properties: Vec<FaceProperty>,
        text: Vec<FaceText>,
        actions: Vec<FaceAction>,
        disclosures: Vec<FaceDisclosure>,
        temporal_references: Vec<TemporalReference>,
        temporal_facts: Vec<FaceTemporalFact>,
    ) -> Result<Self, FaceError> {
        basis.sign_ids.sort();
        if basis.sign_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(FaceError::DuplicateSign);
        }
        let mut value = Self {
            identity: FaceContentId(String::new()),
            revision,
            basis,
            interaction_context: crate::FaceInteractionContext::general(),
            subjects,
            relationships,
            composition: Vec::new(),
            properties,
            text,
            actions,
            disclosures,
            temporal_references,
            temporal_facts,
        };
        value.validate_content()?;
        value.identity = FaceContentId(value.content_digest());
        Ok(value)
    }

    pub(crate) fn validate_temporal(&self) -> Result<(), FaceError> {
        if self.temporal_references.len() > MAX_TEMPORAL_REFERENCES {
            return Err(FaceError::TooManyTemporalReferences);
        }
        if self.temporal_facts.len() > MAX_FACE_TEMPORAL_FACTS {
            return Err(FaceError::TooManyTemporalFacts);
        }
        for (index, reference) in self.temporal_references.iter().enumerate() {
            validate_id(&reference.identity)?;
            map_relation_error(reference.instant.validate())?;
            if self.temporal_references[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == reference.identity)
            {
                return Err(FaceError::DuplicateTemporalReference);
            }
        }
        for fact in &self.temporal_facts {
            if !self.has_subject(&fact.subject) {
                return Err(FaceError::UnknownTemporalSubject);
            }
            if let Some(sign_id) = &fact.sign_id {
                if !self.basis.sign_ids.contains(sign_id) {
                    return Err(FaceError::UnknownTemporalSign);
                }
            }
            let reference = self
                .temporal_references
                .iter()
                .find(|candidate| candidate.identity == fact.reference)
                .ok_or(FaceError::UnknownTemporalReference)?;
            let derived = map_relation_error(fact.source.relation_to(&reference.instant))?;
            if derived != fact.relation {
                return Err(FaceError::InvalidTemporalRelation);
            }
        }
        Ok(())
    }

    pub(crate) fn temporal_len(&self) -> usize {
        self.temporal_references
            .iter()
            .map(|reference| reference.identity.len() + instant_len(&reference.instant))
            .sum::<usize>()
            + self
                .temporal_facts
                .iter()
                .map(|fact| {
                    fact.subject.len()
                        + 1
                        + fact.sign_id.as_ref().map_or(0, |sign| sign.as_str().len())
                        + instant_len(&fact.source)
                        + fact.reference.len()
                        + relation_len(fact.relation)
                })
                .sum::<usize>()
    }

    pub(crate) fn hash_temporal(&self, digest: &mut Sha256) {
        for reference in &self.temporal_references {
            hash_string(digest, &reference.identity);
            hash_instant(digest, &reference.instant);
        }
        for fact in &self.temporal_facts {
            hash_string(digest, &fact.subject);
            digest.update([fact.role as u8]);
            digest.update([u8::from(fact.sign_id.is_some())]);
            if let Some(sign_id) = &fact.sign_id {
                hash_string(digest, sign_id.as_str());
            }
            hash_instant(digest, &fact.source);
            hash_string(digest, &fact.reference);
            hash_relation(digest, fact.relation);
        }
    }
}

fn map_relation_error<T>(result: Result<T, TemporalRelationError>) -> Result<T, FaceError> {
    result.map_err(|error| match error {
        TemporalRelationError::InvalidInstant => FaceError::InvalidTemporalInstant,
        TemporalRelationError::Incomparable => FaceError::IncomparableTemporalInstants,
        TemporalRelationError::IntervalOverflow => FaceError::TemporalIntervalOverflow,
    })
}

fn instant_len(instant: &TemporalInstant) -> usize {
    8 + 1 + instant.clock_basis.len() + 8 + 8
}

fn relation_len(relation: TemporalRelation) -> usize {
    match relation {
        TemporalRelation::Past { .. } | TemporalRelation::Future { .. } => 17,
        TemporalRelation::Present | TemporalRelation::Indeterminate => 1,
    }
}

fn hash_instant(digest: &mut Sha256, instant: &TemporalInstant) {
    digest.update(instant.ticks.to_le_bytes());
    digest.update([instant.scale as u8]);
    hash_string(digest, &instant.clock_basis);
    digest.update(instant.resolution_ticks.to_le_bytes());
    digest.update(instant.uncertainty_ticks.to_le_bytes());
}

fn hash_relation(digest: &mut Sha256, relation: TemporalRelation) {
    match relation {
        TemporalRelation::Past {
            minimum_ticks,
            maximum_ticks,
        } => {
            digest.update([0]);
            digest.update(minimum_ticks.to_le_bytes());
            digest.update(maximum_ticks.to_le_bytes());
        }
        TemporalRelation::Present => digest.update([1]),
        TemporalRelation::Future {
            minimum_ticks,
            maximum_ticks,
        } => {
            digest.update([2]);
            digest.update(minimum_ticks.to_le_bytes());
            digest.update(maximum_ticks.to_le_bytes());
        }
        TemporalRelation::Indeterminate => digest.update([3]),
    }
}
