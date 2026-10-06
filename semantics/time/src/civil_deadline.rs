//! Explicit, conservative conversion of a resolved civil occurrence to one Host deadline.

use conduit_core::{MonotonicDeadline, MonotonicTimeRefusal};

use crate::{CivilResolutionChoice, ClockCorrelation, OccurrenceInstant, ZonedResolution};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CivilDeadlineAdmission {
    occurrence: OccurrenceInstant,
    resolution: ZonedResolution,
    correlation: ClockCorrelation,
    deadline: MonotonicDeadline,
    uncertainty_ticks: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CivilDeadlineRefusal {
    UnresolvedCivilTime,
    InvalidOccurrence,
    ResolutionMismatch,
    InvalidCorrelation,
    DifferentWallBasis,
    DifferentScale,
    UncertainOccurrence,
    UncertaintyExceeded,
    HorizonExceeded,
    OccurrenceNotFuture,
    Overflow,
    Clock(MonotonicTimeRefusal),
}

impl CivilDeadlineAdmission {
    pub fn occurrence(&self) -> &OccurrenceInstant {
        &self.occurrence
    }

    pub fn resolution(&self) -> &ZonedResolution {
        &self.resolution
    }

    pub fn correlation(&self) -> &ClockCorrelation {
        &self.correlation
    }

    pub fn deadline(&self) -> &MonotonicDeadline {
        &self.deadline
    }

    pub const fn uncertainty_ticks(&self) -> u64 {
        self.uncertainty_ticks
    }

    pub fn admit(
        occurrence: OccurrenceInstant,
        resolution: ZonedResolution,
        correlation: ClockCorrelation,
        drift_uncertainty_ticks: u64,
        maximum_uncertainty_ticks: u64,
        maximum_horizon_ticks: u64,
    ) -> Result<Self, CivilDeadlineRefusal> {
        let OccurrenceInstant::Civil(resolved) = &occurrence else {
            return Err(CivilDeadlineRefusal::UnresolvedCivilTime);
        };
        resolved
            .local()
            .validate()
            .map_err(|_| CivilDeadlineRefusal::InvalidOccurrence)?;
        resolved
            .zone()
            .validate()
            .map_err(|_| CivilDeadlineRefusal::InvalidOccurrence)?;
        resolved
            .instant()
            .validate()
            .map_err(|_| CivilDeadlineRefusal::InvalidOccurrence)?;
        resolution
            .validate()
            .map_err(|_| CivilDeadlineRefusal::ResolutionMismatch)?;
        correlation
            .validate()
            .map_err(|_| CivilDeadlineRefusal::InvalidCorrelation)?;

        let target = conduit_core::TemporalInstant::try_from(resolved.instant().clone())
            .map_err(|_| CivilDeadlineRefusal::InvalidOccurrence)?;
        let local = conduit_core::LocalDateTime::try_from(resolved.local().clone())
            .map_err(|_| CivilDeadlineRefusal::InvalidOccurrence)?;
        let zone = conduit_core::NamedTimeZone::try_from(resolved.zone().clone())
            .map_err(|_| CivilDeadlineRefusal::InvalidOccurrence)?;
        let (proof_local, proof_zone, proof_instant, proof_choice) = match &resolution {
            ZonedResolution::Unique {
                local,
                zone,
                instant,
            } => (local, zone, instant, CivilResolutionChoice::Unique),
            ZonedResolution::Ambiguous {
                local,
                zone,
                earlier,
                later,
            } => match resolved.resolution() {
                CivilResolutionChoice::FoldEarlier => {
                    (local, zone, earlier, CivilResolutionChoice::FoldEarlier)
                }
                CivilResolutionChoice::FoldLater => {
                    (local, zone, later, CivilResolutionChoice::FoldLater)
                }
                _ => return Err(CivilDeadlineRefusal::ResolutionMismatch),
            },
            ZonedResolution::Nonexistent {
                local,
                zone,
                gap_before,
                gap_after,
            } => match resolved.resolution() {
                CivilResolutionChoice::GapBefore => {
                    (local, zone, gap_before, CivilResolutionChoice::GapBefore)
                }
                CivilResolutionChoice::GapAfter => {
                    (local, zone, gap_after, CivilResolutionChoice::GapAfter)
                }
                _ => return Err(CivilDeadlineRefusal::ResolutionMismatch),
            },
        };
        if &local != proof_local
            || &zone != proof_zone
            || &target != proof_instant
            || resolved.resolution() != &proof_choice
        {
            return Err(CivilDeadlineRefusal::ResolutionMismatch);
        }
        let sample = correlation.wall();
        if target.clock_basis != sample.clock_basis {
            return Err(CivilDeadlineRefusal::DifferentWallBasis);
        }
        if target.scale != sample.scale || target.scale != correlation.monotonic().clock().scale() {
            return Err(CivilDeadlineRefusal::DifferentScale);
        }
        if target.uncertainty_ticks != 0 {
            return Err(CivilDeadlineRefusal::UncertainOccurrence);
        }
        let uncertainty_ticks = sample
            .uncertainty_ticks
            .checked_add(correlation.wall_uncertainty_ticks())
            .and_then(|ticks| {
                ticks.checked_add(correlation.monotonic().clock().uncertainty_ticks())
            })
            .and_then(|ticks| ticks.checked_add(drift_uncertainty_ticks))
            .ok_or(CivilDeadlineRefusal::Overflow)?;
        if uncertainty_ticks > maximum_uncertainty_ticks {
            return Err(CivilDeadlineRefusal::UncertaintyExceeded);
        }
        let elapsed = target
            .ticks
            .checked_sub(sample.ticks)
            .filter(|ticks| *ticks > uncertainty_ticks)
            .ok_or(CivilDeadlineRefusal::OccurrenceNotFuture)?;
        if elapsed > maximum_horizon_ticks {
            return Err(CivilDeadlineRefusal::HorizonExceeded);
        }
        let latest_due = elapsed
            .checked_add(uncertainty_ticks)
            .ok_or(CivilDeadlineRefusal::Overflow)?;
        let deadline = correlation
            .monotonic()
            .deadline_after(conduit_core::MonotonicDuration::new(
                latest_due,
                target.scale,
            ))
            .map_err(CivilDeadlineRefusal::Clock)?;
        Ok(Self {
            occurrence,
            resolution,
            correlation,
            deadline,
            uncertainty_ticks,
        })
    }

    pub fn remaining_at(
        &self,
        now: &conduit_core::MonotonicInstant,
    ) -> Result<Option<conduit_core::MonotonicDuration>, MonotonicTimeRefusal> {
        self.deadline.remaining_at(now)
    }
}
