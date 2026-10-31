//! Finite collector-observed history for one ordinary shared Face.
//! Observation times align only within the exact collector Host/Boot. Device
//! counters and reported clocks confer neither membership nor synchronization.
use alloc::{string::String, vec::Vec};
use conduit_environment_measurement::source_observation::{ObservationContract, SourceObservation};

pub const SOURCE_SERIES_SCHEMA: &str = "environment/source-observation-series@1";
pub const SOURCE_SERIES_CLOCK: &str = "collector-observed-boot-ms";
pub const MAX_SOURCE_SERIES: usize = 4;
pub const MAX_SOURCE_SERIES_SAMPLES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSeriesRefusal {
    Extent,
    Identity,
    Clock,
    Sequence,
    Observation,
    Face,
    Contract,
}
/// Original complete canonical SourceObservation, not a re-encoded DHT record.
pub struct SourceSeriesRecord<'a> {
    pub sequence: u64,
    pub observation: &'a [u8],
}
pub struct SourceSeries<'a> {
    /// Stable source-session subject. Names and array positions are not identity.
    pub subject: &'a str,
    pub source: &'a str,
    pub name: &'a str,
    pub samples_outside_view: u64,
    pub stale_after_ms: u64,
    pub records: &'a [SourceSeriesRecord<'a>],
}
pub struct SourceSeriesCollection<'a> {
    pub subject: &'a str,
    pub collector_host: &'a str,
    pub collector_boot: &'a str,
    /// Actual collector cutoff. Not a device acquisition or display clock.
    pub observed_through_boot_ms: u64,
    pub series: &'a [SourceSeries<'a>],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSeriesPoint {
    pub sequence: u64,
    pub observation: SourceObservation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedSourceSeries {
    pub subject: String,
    pub source: String,
    pub name: String,
    pub samples_outside_view: u64,
    pub stale_after_ms: u64,
    pub points: Vec<SourceSeriesPoint>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSeriesGraph {
    pub subject: String,
    pub collector_host: String,
    pub collector_boot: String,
    pub observed_through_boot_ms: u64,
    pub series: Vec<PreparedSourceSeries>,
}
pub(crate) fn identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 192 && !value.chars().any(char::is_control)
}
pub(crate) fn name(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 64 && !value.chars().any(char::is_control)
}
impl SourceSeriesCollection<'_> {
    /// Hosted preparation only. This validates content, not source authority,
    /// recording custody, collector clock truth, or selected Host capacity.
    pub fn validate(&self) -> Result<(), SourceSeriesRefusal> {
        use SourceSeriesRefusal as R;
        if !identity(self.subject)
            || !identity(self.collector_host)
            || !identity(self.collector_boot)
        {
            return Err(R::Identity);
        }
        if self.series.len() > MAX_SOURCE_SERIES {
            return Err(R::Extent);
        }
        let contract = ObservationContract::prepare().map_err(|_| R::Contract)?;
        for (index, series) in self.series.iter().enumerate() {
            if !identity(series.subject)
                || series.subject == self.subject
                || !identity(series.source)
                || !name(series.name)
                || self.series[..index]
                    .iter()
                    .any(|old| old.subject == series.subject || old.source == series.source)
            {
                return Err(R::Identity);
            }
            if series.records.len() > MAX_SOURCE_SERIES_SAMPLES || series.stale_after_ms == 0 {
                return Err(R::Extent);
            }
            let mut previous = None;
            for record in series.records {
                let value = contract
                    .decode(record.observation)
                    .map_err(|_| R::Observation)?;
                if value.observed_at_host_boot_ms > self.observed_through_boot_ms {
                    return Err(R::Clock);
                }
                if record.sequence == 0
                    || previous.is_some_and(|(seq, time)| {
                        record.sequence <= seq || value.observed_at_host_boot_ms < time
                    })
                {
                    return Err(R::Sequence);
                }
                previous = Some((record.sequence, value.observed_at_host_boot_ms));
            }
        }
        Ok(())
    }
}

/// Collector-cutoff state, separate from current Face transport availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceSeriesState {
    Empty,
    Unavailable(conduit_environment_measurement::source_observation::SourceFailure),
    Stale { age_ms: u64 },
    Available { age_ms: u64 },
}
impl PreparedSourceSeries {
    pub fn state_at(
        &self,
        collector_cutoff_ms: u64,
    ) -> Result<SourceSeriesState, SourceSeriesRefusal> {
        use conduit_environment_measurement::source_observation::SourceOutcome;
        let Some(last) = self.points.last() else {
            return Ok(SourceSeriesState::Empty);
        };
        let age_ms = collector_cutoff_ms
            .checked_sub(last.observation.observed_at_host_boot_ms)
            .ok_or(SourceSeriesRefusal::Clock)?;
        Ok(match last.observation.outcome {
            SourceOutcome::Unavailable(reason) => SourceSeriesState::Unavailable(reason),
            SourceOutcome::Available(_) if age_ms >= self.stale_after_ms => {
                SourceSeriesState::Stale { age_ms }
            }
            SourceOutcome::Available(_) => SourceSeriesState::Available { age_ms },
        })
    }
}
