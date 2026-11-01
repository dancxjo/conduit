//! Bounded owner-side projection of accepted Source completions. This cache is
//! neither a recorder nor evidence of MCU membership. The native completion
//! owner must call it only after returning the exact admitted Host/lease.
use conduit_core::{ActivePlayId, BaseInstanceId, BootId, CapabilityId, HostId};
use conduit_environment_history::source_series::{
    SourceSeries, SourceSeriesCollection, SourceSeriesRecord, MAX_SOURCE_SERIES,
    MAX_SOURCE_SERIES_SAMPLES,
};
use conduit_environment_host::source_operator::SourceSessionDescriptor;
use conduit_environment_measurement::source_observation::{
    ObservationContract, SourceOutcome, MAXIMUM_BYTES,
};
use conduit_face::{FaceContributionBasis, FaceFragment};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    Scope,
    Capacity,
    Observation,
    Sequence,
    Clock,
    Retired,
    Face,
}

/// Exact descriptive coordinates retained from already admitted equipment.
/// A copied coordinate is never a substitute for native possession or admission.
pub(super) struct CompletionScope<'a> {
    pub host: &'a HostId,
    pub boot: &'a BootId,
    pub session: &'a BaseInstanceId,
    pub capability: &'a CapabilityId,
    pub play: &'a ActivePlayId,
}

pub(super) struct EnvironmentalFaceState {
    host: HostId,
    boot: BootId,
    session: BaseInstanceId,
    capability: CapabilityId,
    basis: FaceContributionBasis,
    subject: String,
    name: String,
    stale_after_ms: u64,
    bytes: [[u8; MAXIMUM_BYTES]; MAX_SOURCE_SERIES_SAMPLES],
    lengths: [usize; MAX_SOURCE_SERIES_SAMPLES],
    sequences: [u64; MAX_SOURCE_SERIES_SAMPLES],
    count: usize,
    next: usize,
    accepted: u64,
    last_now: u64,
    last_observed: u64,
    last_counter: Option<u64>,
    last_reported: Option<u64>,
    active: bool,
    contract: ObservationContract,
}
impl EnvironmentalFaceState {
    pub(super) fn prepare(
        source: &SourceSessionDescriptor,
        basis: FaceContributionBasis,
        subject: String,
        name: String,
        stale_after_ms: u64,
    ) -> Result<Self, Refusal> {
        let valid = |s: &str, max| {
            !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
        };
        if !valid(&subject, 192) || !valid(&name, 64) || stale_after_ms == 0 {
            return Err(Refusal::Scope);
        }
        Ok(Self {
            host: source.host().clone(),
            boot: source.boot().clone(),
            session: source.session().clone(),
            capability: source.capability().clone(),
            basis,
            subject,
            name,
            stale_after_ms,
            bytes: [[0; MAXIMUM_BYTES]; MAX_SOURCE_SERIES_SAMPLES],
            lengths: [0; MAX_SOURCE_SERIES_SAMPLES],
            sequences: [0; MAX_SOURCE_SERIES_SAMPLES],
            count: 0,
            next: 0,
            accepted: 0,
            last_now: 0,
            last_observed: 0,
            last_counter: None,
            last_reported: None,
            active: true,
            contract: ObservationContract::prepare().map_err(|_| Refusal::Observation)?,
        })
    }
    fn scope(&self, scope: &CompletionScope<'_>) -> bool {
        scope.host == &self.host
            && scope.boot == &self.boot
            && scope.session == &self.session
            && scope.capability == &self.capability
            && scope.play == &self.basis.active_play_id
    }
    /// Sequence is the accepted completion order for this retained Source
    /// session; source counters remain correlation, never recording identity.
    /// Validation is transactional; retained sample storage is prepared up front.
    pub(super) fn accept(
        &mut self,
        scope: &CompletionScope<'_>,
        sequence: u64,
        canonical: &[u8],
        now: u64,
    ) -> Result<(), Refusal> {
        if !self.active {
            return Err(Refusal::Retired);
        }
        if !self.scope(scope) {
            return Err(Refusal::Scope);
        }
        if self.accepted.checked_add(1) != Some(sequence) {
            return Err(Refusal::Sequence);
        }
        if canonical.is_empty() || canonical.len() > MAXIMUM_BYTES {
            return Err(Refusal::Capacity);
        }
        let value = self
            .contract
            .decode(canonical)
            .map_err(|_| Refusal::Observation)?;
        if now < self.last_now
            || value.observed_at_host_boot_ms > now
            || value.observed_at_host_boot_ms < self.last_observed
        {
            return Err(Refusal::Clock);
        }
        let coordinates = match value.outcome {
            SourceOutcome::Available(sample) => {
                if self
                    .last_counter
                    .is_some_and(|old| sample.source_counter <= old)
                    || self
                        .last_reported
                        .zip(sample.reported_source_ms)
                        .is_some_and(|(old, next)| next < old)
                {
                    return Err(Refusal::Sequence);
                }
                Some((sample.source_counter, sample.reported_source_ms))
            }
            SourceOutcome::Unavailable(_) => None,
        };
        self.bytes[self.next].fill(0);
        self.bytes[self.next][..canonical.len()].copy_from_slice(canonical);
        self.lengths[self.next] = canonical.len();
        self.sequences[self.next] = sequence;
        self.next = (self.next + 1) % MAX_SOURCE_SERIES_SAMPLES;
        self.count = (self.count + 1).min(MAX_SOURCE_SERIES_SAMPLES);
        self.accepted = sequence;
        self.last_now = now;
        self.last_observed = value.observed_at_host_boot_ms;
        if let Some((counter, reported)) = coordinates {
            self.last_counter = Some(counter);
            if reported.is_some() {
                self.last_reported = reported;
            }
        }
        Ok(())
    }
    /// Cancellation removes the cache's current contribution authority and
    /// erases its samples. Re-admission must prepare a new explicit scope.
    pub(super) fn retire(&mut self, scope: &CompletionScope<'_>) -> Result<(), Refusal> {
        if !self.scope(scope) {
            return Err(Refusal::Scope);
        }
        self.active = false;
        self.bytes.fill([0; MAXIMUM_BYTES]);
        self.lengths.fill(0);
        self.count = 0;
        Ok(())
    }
    fn records(&self) -> Vec<SourceSeriesRecord<'_>> {
        let first =
            (self.next + MAX_SOURCE_SERIES_SAMPLES - self.count) % MAX_SOURCE_SERIES_SAMPLES;
        (0..self.count)
            .map(|offset| {
                let index = (first + offset) % MAX_SOURCE_SERIES_SAMPLES;
                SourceSeriesRecord {
                    sequence: self.sequences[index],
                    observation: &self.bytes[index][..self.lengths[index]],
                }
            })
            .collect()
    }
}

/// Hosted Face preparation only: bounded borrowed rows use the shared graph
/// vocabulary and original canonical values; no observation is re-encoded.
pub(super) fn contribution(
    sources: &[&EnvironmentalFaceState],
    now: u64,
) -> Result<FaceFragment, Refusal> {
    let first = sources.first().ok_or(Refusal::Scope)?;
    if sources.len() > MAX_SOURCE_SERIES {
        return Err(Refusal::Capacity);
    }
    if sources.iter().any(|s| {
        !s.active
            || s.host != first.host
            || s.boot != first.boot
            || s.basis != first.basis
            || now < s.last_now
    }) {
        return Err(Refusal::Scope);
    }
    let records: Vec<_> = sources.iter().map(|s| s.records()).collect();
    let series: Vec<_> = sources
        .iter()
        .zip(&records)
        .map(|(s, rows)| SourceSeries {
            subject: &s.subject,
            source: s.session.as_str(),
            name: &s.name,
            samples_outside_view: s.accepted - s.count as u64,
            stale_after_ms: s.stale_after_ms,
            records: rows,
        })
        .collect();
    SourceSeriesCollection {
        subject: "environmental-observations",
        collector_host: first.host.as_str(),
        collector_boot: first.boot.as_str(),
        observed_through_boot_ms: now,
        series: &series,
    }
    .contribution(first.basis.clone())
    .map_err(|_| Refusal::Face)
}

#[cfg(test)]
#[path = "environmental_face_tests.rs"]
mod tests;
