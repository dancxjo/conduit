//! Reviewed freshness classification for current Experience inputs.

use conduit_core::{TemporalInstant, TemporalRelation, TemporalRelationError};

use crate::ExperienceTemporalRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperienceTemporalPolicy {
    pub maximum_current_age_ticks: u64,
    pub maximum_recent_age_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceTemporalRefusal {
    InvalidPolicy,
    InvalidInstant,
    IncomparableClock,
    IntervalOverflow,
    FutureObservation,
    IndeterminateAge,
}

impl ExperienceTemporalPolicy {
    pub fn validate(self) -> Result<(), ExperienceTemporalRefusal> {
        if self.maximum_recent_age_ticks <= self.maximum_current_age_ticks {
            return Err(ExperienceTemporalRefusal::InvalidPolicy);
        }
        Ok(())
    }

    pub fn classify(
        self,
        observed_at: &TemporalInstant,
        reference_at: &TemporalInstant,
    ) -> Result<ExperienceTemporalRole, ExperienceTemporalRefusal> {
        self.validate()?;
        let relation = observed_at
            .relation_to(reference_at)
            .map_err(map_relation_refusal)?;
        match relation {
            TemporalRelation::Present => Ok(ExperienceTemporalRole::Current),
            TemporalRelation::Past { maximum_ticks, .. }
                if maximum_ticks <= self.maximum_current_age_ticks =>
            {
                Ok(ExperienceTemporalRole::Current)
            }
            TemporalRelation::Past {
                minimum_ticks,
                maximum_ticks,
            } if minimum_ticks > self.maximum_current_age_ticks
                && maximum_ticks <= self.maximum_recent_age_ticks =>
            {
                Ok(ExperienceTemporalRole::Recent)
            }
            TemporalRelation::Past { minimum_ticks, .. }
                if minimum_ticks > self.maximum_recent_age_ticks =>
            {
                Ok(ExperienceTemporalRole::Stale)
            }
            TemporalRelation::Indeterminate
                if maximum_possible_age(observed_at, reference_at)?
                    <= self.maximum_current_age_ticks =>
            {
                Ok(ExperienceTemporalRole::Current)
            }
            TemporalRelation::Past { .. } | TemporalRelation::Indeterminate => {
                Err(ExperienceTemporalRefusal::IndeterminateAge)
            }
            TemporalRelation::Future { .. } => Err(ExperienceTemporalRefusal::FutureObservation),
        }
    }
}

fn maximum_possible_age(
    observed_at: &TemporalInstant,
    reference_at: &TemporalInstant,
) -> Result<u64, ExperienceTemporalRefusal> {
    let observed_lower = observed_at
        .ticks
        .checked_sub(observed_at.uncertainty_ticks)
        .ok_or(ExperienceTemporalRefusal::IntervalOverflow)?;
    let reference_upper = reference_at
        .ticks
        .checked_add(reference_at.uncertainty_ticks)
        .ok_or(ExperienceTemporalRefusal::IntervalOverflow)?;
    Ok(reference_upper.saturating_sub(observed_lower))
}

fn map_relation_refusal(refusal: TemporalRelationError) -> ExperienceTemporalRefusal {
    match refusal {
        TemporalRelationError::InvalidInstant => ExperienceTemporalRefusal::InvalidInstant,
        TemporalRelationError::Incomparable => ExperienceTemporalRefusal::IncomparableClock,
        TemporalRelationError::IntervalOverflow => ExperienceTemporalRefusal::IntervalOverflow,
    }
}
