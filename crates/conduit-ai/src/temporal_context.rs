use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAXIMUM_CLOCK_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockBasis {
    UnixEpochMilliseconds,
    MonotonicMilliseconds { identity: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalProvenance {
    pub event_at: Option<u64>,
    pub valid_from: Option<u64>,
    pub valid_until: Option<u64>,
    pub observed_at: Option<u64>,
    pub recorded_at: Option<u64>,
    pub ingested_at: Option<u64>,
    pub retrieved_at: u64,
    pub reference_at: u64,
    pub clock_basis: ClockBasis,
    pub uncertainty_millis: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalSource {
    Event,
    ValidFrom,
    ValidUntil,
    Observed,
    Recorded,
    Ingested,
    Retrieved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalDirection {
    BeforeReference,
    AtReference,
    AfterReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalRelation {
    pub direction: TemporalDirection,
    pub distance_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityBoundary {
    Created,
    FirstObserved,
    FirstUserMention,
    Born,
    Started,
    CurrentPhaseStarted,
    LastChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionDirection {
    IntoState,
    OutOfState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalRetrievalIntent {
    EarliestEvidence,
    LatestEvidence,
    StateValidAt { instant: u64 },
    Transition { direction: TransitionDirection },
    DurationSince { boundary: EntityBoundary },
    EventOrdering,
    EvidenceWithin { start: u64, end: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalValidity {
    Current,
    Historical,
    Superseded,
    UnknownWhetherCurrent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalContext {
    pub source: TemporalSource,
    pub relation: TemporalRelation,
    pub validity: TemporalValidity,
    pub relation_to_query_window: Option<TemporalWindowRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalWindowRelation {
    Before,
    Within,
    After,
    Overlaps,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalContextRefusal {
    EmptyClockIdentity,
    ClockIdentityTooLarge,
    ReversedValidityInterval,
    RetrievalAfterReference,
    ClockBasisMismatch,
    SourceUnavailable,
    SourceAfterReference,
    ReversedQueryWindow,
    ArithmeticOverflow,
}

impl TemporalProvenance {
    pub fn validate(&self) -> Result<(), TemporalContextRefusal> {
        if let ClockBasis::MonotonicMilliseconds { identity } = &self.clock_basis {
            if identity.is_empty() {
                return Err(TemporalContextRefusal::EmptyClockIdentity);
            }
            if identity.len() > MAXIMUM_CLOCK_IDENTITY_BYTES {
                return Err(TemporalContextRefusal::ClockIdentityTooLarge);
            }
        }
        if self
            .valid_from
            .zip(self.valid_until)
            .is_some_and(|(start, end)| start > end)
        {
            return Err(TemporalContextRefusal::ReversedValidityInterval);
        }
        if self.retrieved_at > self.reference_at {
            return Err(TemporalContextRefusal::RetrievalAfterReference);
        }
        Ok(())
    }

    pub fn relation(
        &self,
        source: TemporalSource,
    ) -> Result<TemporalRelation, TemporalContextRefusal> {
        self.validate()?;
        let source_at = self.source_instant(source)?;
        Ok(relation_between_instants(source_at, self.reference_at))
    }

    pub fn relation_to(
        &self,
        source: TemporalSource,
        other: &Self,
        other_source: TemporalSource,
    ) -> Result<TemporalRelation, TemporalContextRefusal> {
        self.validate()?;
        other.validate()?;
        if self.clock_basis != other.clock_basis {
            return Err(TemporalContextRefusal::ClockBasisMismatch);
        }
        Ok(relation_between_instants(
            self.source_instant(source)?,
            other.source_instant(other_source)?,
        ))
    }

    pub fn age(&self, source: TemporalSource) -> Result<u64, TemporalContextRefusal> {
        let relation = self.relation(source)?;
        match relation.direction {
            TemporalDirection::BeforeReference | TemporalDirection::AtReference => {
                Ok(relation.distance_millis)
            }
            TemporalDirection::AfterReference => Err(TemporalContextRefusal::SourceAfterReference),
        }
    }

    pub fn validity_duration(&self) -> Result<Option<u64>, TemporalContextRefusal> {
        self.validate()?;
        self.valid_from
            .zip(self.valid_until)
            .map(|(start, end)| {
                end.checked_sub(start)
                    .ok_or(TemporalContextRefusal::ArithmeticOverflow)
            })
            .transpose()
    }

    fn source_instant(&self, source: TemporalSource) -> Result<u64, TemporalContextRefusal> {
        let instant = match source {
            TemporalSource::Event => self.event_at,
            TemporalSource::ValidFrom => self.valid_from,
            TemporalSource::ValidUntil => self.valid_until,
            TemporalSource::Observed => self.observed_at,
            TemporalSource::Recorded => self.recorded_at,
            TemporalSource::Ingested => self.ingested_at,
            TemporalSource::Retrieved => Some(self.retrieved_at),
        };
        instant.ok_or(TemporalContextRefusal::SourceUnavailable)
    }
}

fn relation_between_instants(source_at: u64, reference_at: u64) -> TemporalRelation {
    match source_at.cmp(&reference_at) {
        core::cmp::Ordering::Less => TemporalRelation {
            direction: TemporalDirection::BeforeReference,
            distance_millis: reference_at - source_at,
        },
        core::cmp::Ordering::Equal => TemporalRelation {
            direction: TemporalDirection::AtReference,
            distance_millis: 0,
        },
        core::cmp::Ordering::Greater => TemporalRelation {
            direction: TemporalDirection::AfterReference,
            distance_millis: source_at - reference_at,
        },
    }
}

impl TemporalRetrievalIntent {
    pub fn validate(&self) -> Result<(), TemporalContextRefusal> {
        if let Self::EvidenceWithin { start, end } = self {
            if start > end {
                return Err(TemporalContextRefusal::ReversedQueryWindow);
            }
        }
        Ok(())
    }
}
