use alloc::string::String;
use conduit_core::{TemporalInstant, TemporalRelation, TemporalRelationError, TemporalScale};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_CLOCK_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockBasis {
    UnixEpochMilliseconds,
    MonotonicMilliseconds { identity: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalReference {
    pub reference_at: u64,
    pub clock_basis: ClockBasis,
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
    UncertainAge,
}

impl TemporalProvenance {
    pub fn validate(&self) -> Result<(), TemporalContextRefusal> {
        validate_clock_basis(&self.clock_basis)?;
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
        let source = self.canonical_source_instant(source)?;
        let reference = self.canonical_reference_instant()?;
        source.relation_to(&reference).map_err(map_relation_error)
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
        self.canonical_source_instant(source)?
            .relation_to(&other.canonical_source_instant(other_source)?)
            .map_err(map_relation_error)
    }

    pub fn age(&self, source: TemporalSource) -> Result<u64, TemporalContextRefusal> {
        let relation = self.relation(source)?;
        match relation {
            TemporalRelation::Past {
                minimum_ticks,
                maximum_ticks,
            } if minimum_ticks == maximum_ticks => Ok(minimum_ticks),
            TemporalRelation::Present => Ok(0),
            TemporalRelation::Future { .. } => Err(TemporalContextRefusal::SourceAfterReference),
            TemporalRelation::Past { .. } | TemporalRelation::Indeterminate => {
                Err(TemporalContextRefusal::UncertainAge)
            }
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

    pub fn source_instant(&self, source: TemporalSource) -> Result<u64, TemporalContextRefusal> {
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

    pub fn canonical_source_instant(
        &self,
        source: TemporalSource,
    ) -> Result<TemporalInstant, TemporalContextRefusal> {
        self.validate()?;
        Ok(TemporalInstant {
            ticks: self.source_instant(source)?,
            scale: TemporalScale::Milliseconds,
            clock_basis: canonical_clock_basis(&self.clock_basis),
            resolution_ticks: 1,
            uncertainty_ticks: self.uncertainty_millis.unwrap_or(0),
        })
    }

    pub fn canonical_reference_instant(&self) -> Result<TemporalInstant, TemporalContextRefusal> {
        self.validate()?;
        Ok(TemporalInstant {
            ticks: self.reference_at,
            scale: TemporalScale::Milliseconds,
            clock_basis: canonical_clock_basis(&self.clock_basis),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        })
    }
}

impl TemporalReference {
    pub fn validate(&self) -> Result<(), TemporalContextRefusal> {
        validate_clock_basis(&self.clock_basis)
    }
}

fn validate_clock_basis(clock_basis: &ClockBasis) -> Result<(), TemporalContextRefusal> {
    if let ClockBasis::MonotonicMilliseconds { identity } = clock_basis {
        if identity.is_empty() {
            return Err(TemporalContextRefusal::EmptyClockIdentity);
        }
        if identity.len() > MAXIMUM_CLOCK_IDENTITY_BYTES {
            return Err(TemporalContextRefusal::ClockIdentityTooLarge);
        }
    }
    Ok(())
}

fn canonical_clock_basis(clock_basis: &ClockBasis) -> String {
    match clock_basis {
        ClockBasis::UnixEpochMilliseconds => String::from(conduit_core::UNIX_UTC_CLOCK_BASIS),
        ClockBasis::MonotonicMilliseconds { identity } => identity.clone(),
    }
}

fn map_relation_error(error: TemporalRelationError) -> TemporalContextRefusal {
    match error {
        TemporalRelationError::Incomparable => TemporalContextRefusal::ClockBasisMismatch,
        TemporalRelationError::InvalidInstant | TemporalRelationError::IntervalOverflow => {
            TemporalContextRefusal::ArithmeticOverflow
        }
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
