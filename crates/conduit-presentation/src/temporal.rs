//! Bounded temporal context without ambient clocks or presentation policy.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::SignId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::identity::hash_string;
use crate::presentation::validate_id;
use crate::{
    Presentation, PresentationAction, PresentationBasis, PresentationContentId,
    PresentationDisclosure, PresentationError, PresentationProperty, PresentationRelationship,
    PresentationSubject, PresentationText,
};

pub const MAX_TEMPORAL_REFERENCES: usize = 256;
pub const MAX_PRESENTATION_TEMPORAL_FACTS: usize = 1_024;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalScale {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalInstant {
    pub ticks: u64,
    pub scale: TemporalScale,
    pub clock_basis: String,
    pub resolution_ticks: u64,
    pub uncertainty_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalReference {
    pub identity: String,
    pub instant: TemporalInstant,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresentationTemporalRole {
    Event,
    Observation,
    Ingestion,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalRelation {
    Past {
        minimum_ticks: u64,
        maximum_ticks: u64,
    },
    Present,
    Future {
        minimum_ticks: u64,
        maximum_ticks: u64,
    },
    Indeterminate,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TemporalRelationError {
    InvalidInstant,
    Incomparable,
    IntervalOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresentationTemporalFact {
    pub subject: String,
    pub role: PresentationTemporalRole,
    pub sign_id: Option<SignId>,
    pub source: TemporalInstant,
    pub reference: String,
    pub relation: TemporalRelation,
}

impl TemporalInstant {
    pub fn relation_to(
        &self,
        reference: &TemporalInstant,
    ) -> Result<TemporalRelation, TemporalRelationError> {
        validate_instant(self)?;
        validate_instant(reference)?;
        if self.clock_basis != reference.clock_basis || self.scale != reference.scale {
            return Err(TemporalRelationError::Incomparable);
        }
        let source = interval(self)?;
        let target = interval(reference)?;
        if source.1 < target.0 {
            return Ok(TemporalRelation::Past {
                minimum_ticks: target.0 - source.1,
                maximum_ticks: target.1 - source.0,
            });
        }
        if source.0 > target.1 {
            return Ok(TemporalRelation::Future {
                minimum_ticks: source.0 - target.1,
                maximum_ticks: source.1 - target.0,
            });
        }
        if self.ticks == reference.ticks
            && self.uncertainty_ticks == 0
            && reference.uncertainty_ticks == 0
        {
            Ok(TemporalRelation::Present)
        } else {
            Ok(TemporalRelation::Indeterminate)
        }
    }
}

impl PresentationTemporalFact {
    pub fn new(
        subject: String,
        role: PresentationTemporalRole,
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

impl Presentation {
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_semantics_and_temporal(
        revision: u64,
        mut basis: PresentationBasis,
        subjects: Vec<PresentationSubject>,
        relationships: Vec<PresentationRelationship>,
        properties: Vec<PresentationProperty>,
        text: Vec<PresentationText>,
        actions: Vec<PresentationAction>,
        disclosures: Vec<PresentationDisclosure>,
        temporal_references: Vec<TemporalReference>,
        temporal_facts: Vec<PresentationTemporalFact>,
    ) -> Result<Self, PresentationError> {
        basis.sign_ids.sort();
        if basis.sign_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(PresentationError::DuplicateSign);
        }
        let mut value = Self {
            identity: PresentationContentId(String::new()),
            revision,
            basis,
            subjects,
            relationships,
            properties,
            text,
            actions,
            disclosures,
            temporal_references,
            temporal_facts,
        };
        value.validate_content()?;
        value.identity = PresentationContentId(value.content_digest());
        Ok(value)
    }

    pub(crate) fn validate_temporal(&self) -> Result<(), PresentationError> {
        if self.temporal_references.len() > MAX_TEMPORAL_REFERENCES {
            return Err(PresentationError::TooManyTemporalReferences);
        }
        if self.temporal_facts.len() > MAX_PRESENTATION_TEMPORAL_FACTS {
            return Err(PresentationError::TooManyTemporalFacts);
        }
        for (index, reference) in self.temporal_references.iter().enumerate() {
            validate_id(&reference.identity)?;
            map_relation_error(validate_instant(&reference.instant))?;
            if self.temporal_references[index + 1..]
                .iter()
                .any(|candidate| candidate.identity == reference.identity)
            {
                return Err(PresentationError::DuplicateTemporalReference);
            }
        }
        for fact in &self.temporal_facts {
            if !self.has_subject(&fact.subject) {
                return Err(PresentationError::UnknownTemporalSubject);
            }
            if let Some(sign_id) = &fact.sign_id {
                if !self.basis.sign_ids.contains(sign_id) {
                    return Err(PresentationError::UnknownTemporalSign);
                }
            }
            let reference = self
                .temporal_references
                .iter()
                .find(|candidate| candidate.identity == fact.reference)
                .ok_or(PresentationError::UnknownTemporalReference)?;
            let derived = map_relation_error(fact.source.relation_to(&reference.instant))?;
            if derived != fact.relation {
                return Err(PresentationError::InvalidTemporalRelation);
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

fn validate_instant(instant: &TemporalInstant) -> Result<(), TemporalRelationError> {
    if instant.clock_basis.is_empty()
        || instant.clock_basis.len() > crate::MAX_PRESENTATION_ID_BYTES
        || instant.resolution_ticks == 0
    {
        Err(TemporalRelationError::InvalidInstant)
    } else {
        Ok(())
    }
}

fn interval(instant: &TemporalInstant) -> Result<(u64, u64), TemporalRelationError> {
    let lower = instant
        .ticks
        .checked_sub(instant.uncertainty_ticks)
        .ok_or(TemporalRelationError::IntervalOverflow)?;
    let upper = instant
        .ticks
        .checked_add(instant.uncertainty_ticks)
        .ok_or(TemporalRelationError::IntervalOverflow)?;
    Ok((lower, upper))
}

fn map_relation_error<T>(result: Result<T, TemporalRelationError>) -> Result<T, PresentationError> {
    result.map_err(|error| match error {
        TemporalRelationError::InvalidInstant => PresentationError::InvalidTemporalInstant,
        TemporalRelationError::Incomparable => PresentationError::IncomparableTemporalInstants,
        TemporalRelationError::IntervalOverflow => PresentationError::TemporalIntervalOverflow,
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
