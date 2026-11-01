//! Prepared bounded view of accepted Source Fore; never a durable recorder.
use super::continuing::SourceSelection;
use super::*;
use conduit_environment_history::source_series::{
    SourceSeries, SourceSeriesCollection, SourceSeriesRecord,
};
use conduit_environment_measurement::source_observation::{
    ObservationContract, SourceOutcome, MAXIMUM_BYTES,
};
use conduit_face::{FaceContributionBasis, FaceFragment};
use std::sync::Arc;

struct Source {
    selection: Option<Arc<SourceSelection>>,
    subject: String,
    bytes: [[u8; MAXIMUM_BYTES]; 16],
    lengths: [usize; 16],
    sequences: [u64; 16],
    next: usize,
    next_fore: u64,
    accepted: u64,
    observed: u64,
    counter: Option<u64>,
    reported: Option<u64>,
}
impl Source {
    fn empty() -> Self {
        Self {
            selection: None,
            subject: String::new(),
            bytes: [[0; MAXIMUM_BYTES]; 16],
            lengths: [0; 16],
            sequences: [0; 16],
            next: 1,
            next_fore: 0,
            accepted: 0,
            observed: 0,
            counter: None,
            reported: None,
        }
    }
    fn same_session(&self, s: &SourceSelection) -> bool {
        self.selection.as_ref().is_some_and(|old| {
            old.attachment_id == s.attachment_id
                && old.host_id == s.host_id
                && old.boot_id == s.boot_id
                && old.capability_id == s.capability_id
                && old.provider_instance_id == s.provider_instance_id
                && old.provider_generation == s.provider_generation
                && old.configuration_id == s.configuration_id
                && old.firmware_sha256 == s.firmware_sha256
        })
    }
    fn store(&mut self, index: usize, sequence: u64, bytes: &[u8]) {
        self.bytes[index].fill(0);
        self.bytes[index][..bytes.len()].copy_from_slice(bytes);
        self.lengths[index] = bytes.len();
        self.sequences[index] = sequence;
    }
    fn records(&self) -> Vec<SourceSeriesRecord<'_>> {
        let mut rows: Vec<_> = (1..16).filter(|&i| self.lengths[i] != 0).collect();
        if self.lengths[0] != 0 && !rows.iter().any(|&i| self.sequences[i] == self.sequences[0]) {
            rows.push(0);
        }
        rows.sort_by_key(|&i| self.sequences[i]);
        rows.into_iter()
            .map(|i| SourceSeriesRecord {
                sequence: self.sequences[i],
                observation: &self.bytes[i][..self.lengths[i]],
            })
            .collect()
    }
}
pub(super) struct EnvironmentalFace {
    sources: [Source; 4],
    contract: ObservationContract,
    last_now: u64,
}
impl EnvironmentalFace {
    pub(super) fn prepare() -> Result<Self, String> {
        Ok(Self {
            sources: std::array::from_fn(|_| Source::empty()),
            contract: ObservationContract::prepare().map_err(debug)?,
            last_now: 0,
        })
    }
    /// Called during preparation, not from per-sample execution. A new child
    /// Play resets Fore order, while the retained provider session keeps counters.
    pub(super) fn select(&mut self, selection: Arc<SourceSelection>) -> Result<(), String> {
        if selection.attachment_id.trim().is_empty()
            || selection.attachment_id.len() > 64
            || selection.attachment_id.chars().any(char::is_control)
        {
            return Err("Source display name exceeds the shared Face contract".into());
        }
        let index = self
            .sources
            .iter()
            .position(|s| s.same_session(&selection))
            .or_else(|| self.sources.iter().position(|s| s.selection.is_none()))
            .ok_or("environmental Face has four retained source sessions")?;
        if self.sources[index].selection.is_none() {
            use sha2::Digest;
            // Descriptive subject only, never execution or membership authority.
            // Child Plan/Play coordinates deliberately do not change the session.
            let coordinates = serde_json::to_vec(&(
                &selection.attachment_id,
                &selection.host_id,
                &selection.boot_id,
                &selection.capability_id,
                &selection.provider_instance_id,
                selection.provider_generation,
                &selection.configuration_id,
                selection.firmware_sha256,
            ))
            .map_err(debug)?;
            self.sources[index].subject = format!(
                "environment/source-session/{:x}",
                sha2::Sha256::digest(coordinates)
            );
        }
        self.sources[index].selection = Some(selection);
        self.sources[index].next_fore = 0;
        Ok(())
    }
    fn accept(&mut self, value: &continuing::native::LiveFore, now: u64) -> Result<(), String> {
        let source = self
            .sources
            .iter_mut()
            .find(|s| {
                s.selection
                    .as_ref()
                    .is_some_and(|selection| **selection == *value.scope.selection)
            })
            .ok_or("live Source differs from exact prepared selection")?;
        if value.sequence != source.next_fore || value.length == 0 || value.length > MAXIMUM_BYTES {
            return Err("live Source Fore order or extent refused".into());
        }
        let canonical = &value.bytes[..value.length];
        let decoded = self.contract.decode(canonical).map_err(debug)?;
        if now < self.last_now
            || decoded.observed_at_host_boot_ms > now
            || decoded.observed_at_host_boot_ms < source.observed
        {
            return Err("Source collector clock refused".into());
        }
        let coordinates = match decoded.outcome {
            SourceOutcome::Available(sample) => {
                if source
                    .counter
                    .is_some_and(|old| sample.source_counter <= old)
                    || source
                        .reported
                        .zip(sample.reported_source_ms)
                        .is_some_and(|(old, new)| new < old)
                {
                    return Err("Source counter or reported coordinate regressed".into());
                }
                Some((sample.source_counter, sample.reported_source_ms))
            }
            SourceOutcome::Unavailable(_) => None,
        };
        let sequence = source
            .accepted
            .checked_add(1)
            .ok_or("Source cache sequence exhausted")?;
        let next = source
            .next_fore
            .checked_add(1)
            .ok_or("Source Fore sequence exhausted")?;
        // Slot zero pins last-good. The other fifteen retain recent outcomes;
        // repeated unavailability or cancellation cannot erase the last reading.
        if coordinates.is_some() {
            source.store(0, sequence, canonical);
        }
        source.store(source.next, sequence, canonical);
        source.next = 1 + source.next % 15;
        source.accepted = sequence;
        source.next_fore = next;
        source.observed = decoded.observed_at_host_boot_ms;
        if let Some((counter, reported)) = coordinates {
            source.counter = Some(counter);
            if reported.is_some() {
                source.reported = reported;
            }
        }
        self.last_now = now;
        Ok(())
    }
    fn contribution(
        &self,
        host: &str,
        boot: &str,
        now: u64,
        basis: FaceContributionBasis,
    ) -> Result<Option<FaceFragment>, String> {
        if now < self.last_now {
            return Err("collector cutoff regressed".into());
        }
        let sources: Vec<_> = self.sources.iter().filter(|s| s.accepted != 0).collect();
        if sources.is_empty() {
            return Ok(None);
        }
        let records: Vec<_> = sources.iter().map(|s| s.records()).collect();
        let series: Vec<_> = sources
            .iter()
            .zip(&records)
            .map(|(s, rows)| {
                let selection = s.selection.as_ref().expect("accepted selected source");
                SourceSeries {
                    subject: &s.subject,
                    source: &s.subject,
                    name: &selection.attachment_id,
                    samples_outside_view: s.accepted - rows.len() as u64,
                    stale_after_ms: 90000,
                    records: rows,
                }
            })
            .collect();
        SourceSeriesCollection {
            subject: "environmental-observations",
            collector_host: host,
            collector_boot: boot,
            observed_through_boot_ms: now,
            series: &series,
        }
        .contribution(basis)
        .map(Some)
        .map_err(debug)
    }
}
/// CLOCK_BOOTTIME matches both installed Source providers, including suspend.
/// This has no wall-time or cross-Host synchronization claim.
fn now() -> Result<u64, String> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut time) } != 0
        || time.tv_sec < 0
        || time.tv_nsec < 0
    {
        return Err("collector CLOCK_BOOTTIME unavailable".into());
    }
    (time.tv_sec as u64)
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add(time.tv_nsec as u64 / 1_000_000))
        .ok_or("collector clock overflow".into())
}
impl Owner {
    pub(super) fn accept_live_source_fore(
        &mut self,
        value: continuing::native::LiveFore,
    ) -> Result<(), String> {
        let realization = self
            .session
            .realization()
            .ok_or("Source Fore has no current Plan")?;
        let play = realization
            .play
            .as_ref()
            .ok_or("Source Fore has no current Play")?;
        let s = &value.scope.selection;
        if !value.scope.native_play.is_bound_to(
            &s.plan_id,
            &s.host_id,
            &s.boot_id,
            play.play_sequence,
        ) || !realization.plan.plots.iter().any(|plot| {
            plot.plan.fragments.iter().any(|fragment| {
                fragment.plan_id == s.plan_id
                    && fragment.fragment_id == s.fragment_id
                    && fragment.host_id == s.host_id
                    && fragment.boot_id == s.boot_id
                    && fragment.placements.iter().any(|p| {
                        p.placement_id == s.placement_id
                            && p.capability_id == s.capability_id
                            && p.base.as_ref().is_some_and(|b| {
                                b.provider_instance_id == s.provider_instance_id
                                    && b.provider_generation == s.provider_generation
                            })
                    })
            })
        }) {
            return Err("Source Fore does not belong to current exact native Play".into());
        }
        self.environmental_face
            .as_mut()
            .ok_or("Source Face slots unprepared")?
            .accept(&value, now()?)
    }
    pub(super) fn environmental_contribution(&self) -> Result<Option<FaceFragment>, String> {
        let Some(cache) = &self.environmental_face else {
            return Ok(None);
        };
        let Some(realization) = self.session.realization() else {
            return Ok(None);
        };
        let Some(play) = &realization.play else {
            return Ok(None);
        };
        let Some(resident) = &self.resident else {
            return Ok(None);
        };
        // Only the actual Source Plot may contribute this view. Cached values
        // survive retirement but confer no current Play authority themselves.
        if !realization.plan.plots.iter().any(|p| {
            p.plot == *resident
                && p.plan.fragments.iter().any(|f| {
                    f.placements.iter().any(|p| {
                        p.kind_id.as_str()
                            == conduit_environment_measurement::source_observation::SOURCE_KIND
                    })
                })
        }) {
            return Ok(None);
        }
        let host = self.host.advertisement();
        cache.contribution(
            host.host_id.as_str(),
            host.boot_id.as_str(),
            now()?,
            FaceContributionBasis {
                checked_plot_id: resident.checked_plot_id.clone(),
                plan_id: realization.plan.plan_id.clone(),
                active_play_id: play.active_play_id.clone(),
                required_interaction_context: None,
            },
        )
    }
}

#[cfg(test)]
#[path = "environmental_face_tests.rs"]
mod tests;
