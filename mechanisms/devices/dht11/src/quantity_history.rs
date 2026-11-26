//! Pure allocating preparation of reported channels from the existing recorder.
//! No I/O, publication observation, authority, receipt, or durable cursor lives here.
use crate::recording::{
    PreparedRecordingHistory, RecordingContext, RecordingDisposition as D, RecordingHistoryRefusal,
};
use crate::ObservationContract;
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{validate_canonical_structured_value, Quantity};
use conduit_data::{
    validate_quantity_observation_snapshot, QuantityObservationGap, QuantityObservationOutcome,
    ResolvedQuantityObservation, ResolvedQuantityObservationBatch,
    ResolvedQuantityObservationSnapshot,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

/// Domain-selected identities; every source and channel must be distinct.
#[derive(Clone, Copy)]
pub struct EnvironmentQuantityChannel<'a> {
    pub source_identity: &'a str,
    pub source_name: &'a str,
    pub channel_identity: &'a str,
    pub channel_name: &'a str,
}
#[derive(Debug, PartialEq, Eq)]
pub enum EnvironmentQuantityRefusal {
    Window,
    Identity,
    Recording(RecordingHistoryRefusal),
    Observation,
    Encoding,
    Extent,
}

/// Owns the existing one-chunk recovery workspace, not another history store.
/// Context is copied from the same initialization used by its validating reader;
/// an unrelated caller-supplied context cannot relabel an already loaded reader.
/// All allocation belongs in separately funded hosted preparation, never a Step.
pub struct PreparedEnvironmentQuantityObservations {
    history: PreparedRecordingHistory,
    producer: [String; 4],
    channels: [[String; 4]; 2],
    tail: usize,
}
impl PreparedEnvironmentQuantityObservations {
    /// Reachable heap only; excludes inline Self, caller contract and input JSON.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self.history.retained_backing_bytes()?;
        for text in self.producer.iter().chain(self.channels.iter().flatten()) {
            bytes = bytes.checked_add(text.capacity())?;
        }
        Some(bytes)
    }
    /// One-generation conversion preparation, including encoding/final frozen
    /// material but excluding caller-owned checked catalog/schema construction,
    /// selected file/read buffers, native worker backing and inline Self.
    /// The successful recorder contract restricts producer IDs to128B; channel
    /// text has eight128B limits. Requested String clones fit those exact sizes.
    pub fn preparation_backing_bound(contract: &ObservationContract) -> Option<usize> {
        PreparedRecordingHistory::preparation_backing_bound(contract)?
            .checked_add(4 * 128 + 8 * 128)?
            .checked_add(Self::encoding_workspace_bound()?)
    }
    /// Schema-bounded generated conversion and final encoding workspace.
    pub fn encoding_workspace_bound() -> Option<usize> {
        crate::quantity_history_backing::conversion_bound()
    }

    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        maximum_chunks: u16,
        temperature: EnvironmentQuantityChannel<'_>,
        relative_humidity: EnvironmentQuantityChannel<'_>,
        tail: usize,
    ) -> Result<Self, EnvironmentQuantityRefusal> {
        use EnvironmentQuantityRefusal as E;
        if !(1..=16).contains(&tail) {
            return Err(E::Window);
        }
        let channels = [temperature, relative_humidity].map(|channel| {
            [
                channel.source_identity,
                channel.source_name,
                channel.channel_identity,
                channel.channel_name,
            ]
        });
        if channels
            .iter()
            .flatten()
            .any(|s| s.is_empty() || s.len() > 128)
        {
            return Err(E::Identity);
        }
        let identities = [
            channels[0][0],
            channels[0][2],
            channels[1][0],
            channels[1][2],
        ];
        for (index, identity) in identities.iter().enumerate() {
            if identities[..index].contains(identity) {
                return Err(E::Identity);
            }
        }
        let producer = [
            context.host.as_str(),
            context.boot.as_str(),
            context.plan.as_str(),
            context.play.as_str(),
        ];
        if producer
            .iter()
            .any(|value| value.is_empty() || value.len() > 128)
        {
            return Err(E::Identity);
        }
        let producer = producer.map(String::from);
        let history = PreparedRecordingHistory::prepare(context, contract, maximum_chunks)
            .map_err(E::Recording)?;
        Ok(Self {
            history,
            producer,
            channels: channels.map(|c| c.map(String::from)),
            tail,
        })
    }
    /// Content validity only; caller independently admits and performs any read.
    pub fn read_next(&mut self, json: &str) -> Result<(), EnvironmentQuantityRefusal> {
        self.history
            .read_next(json)
            .map_err(EnvironmentQuantityRefusal::Recording)
    }
    /// Latest loaded chunk's explicitly selected tail; preceding recovered samples
    /// are counted outside the view. History before recovery remains unknown.
    /// An overlarge encoding refuses rather than shrinking the selected window.
    pub fn snapshot(
        &mut self,
    ) -> Result<ResolvedQuantityObservationSnapshot, EnvironmentQuantityRefusal> {
        use EnvironmentQuantityRefusal as E;
        let loaded = self.history.loaded_samples();
        let start = loaded.saturating_sub(self.tail);
        let visible = loaded - start;
        let outside = u64::from(self.history.recovered().samples)
            .checked_sub(visible as u64)
            .ok_or(E::Observation)?;
        let mut channels: [Vec<ResolvedQuantityObservation>; 2] =
            core::array::from_fn(|_| Vec::with_capacity(visible));
        for index in start..loaded {
            let sample = self.history.sample(index).ok_or(E::Observation)?;
            let outcomes = match (sample.disposition, sample.observation) {
                (D::Available, Some(bytes)) => {
                    let observation =
                        validate_canonical_structured_value(bytes).map_err(|_| E::Observation)?;
                    let quantity = |field| {
                        let value = observation.record_field(field).ok()??;
                        Quantity::decode(value.value_node().get(5..)?).ok()
                    };
                    [quantity("temperature"), quantity("humidity")].map(|q| {
                        q.map(|q| QuantityObservationOutcome::Available(Box::new(q)))
                            .ok_or(E::Observation)
                    })
                }
                (D::Available, None) | (_, Some(_)) => return Err(E::Observation),
                (failure, None) => {
                    let reason = failure_label(failure).ok_or(E::Observation)?;
                    core::array::from_fn(|_| {
                        Ok(QuantityObservationOutcome::Unavailable(reason.into()))
                    })
                }
            };
            for (channel, outcome) in channels.iter_mut().zip(outcomes) {
                channel.push(ResolvedQuantityObservation {
                    sequence: sample.sequence,
                    sampled_at_boot_ms: sample.sampled_at_boot_ms,
                    outcome: outcome?,
                    gap_before: sample.gap_before.map(|(first_sequence, last_sequence)| {
                        QuantityObservationGap {
                            first_sequence,
                            last_sequence,
                        }
                    }),
                });
            }
        }
        let mut batches = Vec::with_capacity(2);
        for (observations, names) in channels.into_iter().zip(&self.channels) {
            let batch = ResolvedQuantityObservationBatch {
                source_identity: names[0].clone(),
                source_name: names[1].clone(),
                channel_identity: names[2].clone(),
                channel_name: names[3].clone(),
                producer_host: self.producer[0].clone(),
                producer_boot: self.producer[1].clone(),
                producer_plan: self.producer[2].clone(),
                producer_play: self.producer[3].clone(),
                samples_outside_view: outside,
                observations: BoundedSequence::try_from_iter(observations)
                    .map_err(|_| E::Encoding)?,
            };
            if batch.clone().encode().map_err(|_| E::Encoding)?.len() > 4096 {
                return Err(E::Extent);
            }
            batches.push(batch);
        }
        let snapshot = ResolvedQuantityObservationSnapshot {
            batches: BoundedSequence::try_from_iter(batches).map_err(|_| E::Encoding)?,
        };
        validate_quantity_observation_snapshot(&snapshot).map_err(|_| E::Encoding)?;
        let bound = conduit_data::resolved_quantity_observation_snapshot_byte_bound()
            .map_err(|_| E::Encoding)?;
        if snapshot.clone().encode().map_err(|_| E::Encoding)?.len() > bound as usize {
            return Err(E::Extent);
        }
        Ok(snapshot)
    }
}
fn failure_label(disposition: D) -> Option<&'static str> {
    Some(match disposition {
        D::Available => return None,
        D::CaptureTimeout => "capture-timeout",
        D::ProviderFailed => "provider-failed",
        D::GeometryRefused => "geometry-refused",
        D::ShortFrame => "short-frame",
        D::MalformedLength => "malformed-length",
        D::Checksum => "checksum",
        D::InvalidPayload => "invalid-payload",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::PreparedRecordingChunk;

    fn channel(source: &'static str, channel: &'static str) -> EnvironmentQuantityChannel<'static> {
        EnvironmentQuantityChannel {
            source_identity: source,
            source_name: source,
            channel_identity: channel,
            channel_name: channel,
        }
    }
    fn fixture(tail: usize) -> (PreparedEnvironmentQuantityObservations, String) {
        let (host, boot, plan, play) = (
            "host/reported".into(),
            "boot/reported".into(),
            "plan/reported".into(),
            "play/reported".into(),
        );
        let context = || RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        };
        let contract = ObservationContract::prepare().unwrap();
        let prepared = PreparedEnvironmentQuantityObservations::prepare(
            context(),
            &contract,
            2,
            channel("source/t", "channel/t"),
            channel("source/h", "channel/h"),
            tail,
        )
        .unwrap();
        let mut forms = crate::PreparedObservationQuantityForms::prepare(&contract).unwrap();
        let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
        let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
        for (sequence, time, disposition) in [
            (1, 0, D::Available),
            (4, 7, D::Checksum),
            (5, 101, D::Available),
        ] {
            chunk
                .append(
                    sequence,
                    time,
                    disposition,
                    (disposition == D::Available).then_some(observation.as_slice()),
                )
                .unwrap();
        }
        (prepared, chunk.seal().unwrap().into())
    }
    #[test]
    fn explicit_tail_preserves_failure_gap_clock_and_outside_count() {
        let (mut prepared, chunk) = fixture(2);
        prepared.read_next(&chunk).unwrap();
        let snapshot = prepared.snapshot().unwrap();
        for batch in snapshot.batches.iter() {
            assert_eq!(batch.producer_boot, "boot/reported");
            assert_eq!(batch.samples_outside_view, 1);
            let samples: Vec<_> = batch.observations.iter().collect();
            assert_eq!((samples[0].sequence, samples[0].sampled_at_boot_ms), (4, 7));
            let gap = samples[0].gap_before.as_ref().unwrap();
            assert_eq!((gap.first_sequence, gap.last_sequence), (2, 3));
            assert_eq!(
                samples[0].outcome,
                QuantityObservationOutcome::Unavailable("checksum".into())
            );
            assert_eq!(samples[1].sampled_at_boot_ms, 101);
            assert!(samples[1].gap_before.is_none());
        }
        let mut forms = crate::PreparedObservationQuantityForms::prepare(
            &ObservationContract::prepare().unwrap(),
        )
        .unwrap();
        let value =
            validate_canonical_structured_value(forms.unpack(&[198, 0, 78, 2]).unwrap()).unwrap();
        for (batch, field) in snapshot.batches.iter().zip(["temperature", "humidity"]) {
            let leaf = value.record_field(field).unwrap().unwrap();
            let expected = Quantity::decode(&leaf.value_node()[5..]).unwrap();
            assert_eq!(
                batch.observations.iter().nth(1).unwrap().outcome,
                QuantityObservationOutcome::Available(Box::new(expected))
            );
        }
    }
    #[test]
    fn foreign_context_refusal_preserves_visible_history() {
        let (mut prepared, chunk) = fixture(16);
        prepared.read_next(&chunk).unwrap();
        let before = prepared.snapshot().unwrap().encode().unwrap();
        assert!(prepared
            .read_next(&chunk.replace("686f73742f7265706f72746564", "686f73742f666f726569676e21"))
            .is_err());
        assert_eq!(prepared.snapshot().unwrap().encode().unwrap(), before);
    }
    #[test]
    fn names_cannot_alias_derived_sample_subjects() {
        let (mut prepared, chunk) = fixture(16);
        prepared.channels[1][0] = "source/t/sample/1".into();
        prepared.read_next(&chunk).unwrap();
        assert_eq!(
            prepared.snapshot().err(),
            Some(EnvironmentQuantityRefusal::Encoding)
        );
    }
    #[test]
    fn preparation_refuses_implicit_window_and_duplicate_identities() {
        let (host, boot, plan, play) = (
            "host/test".into(),
            "boot/test".into(),
            "plan/test".into(),
            "play/test".into(),
        );
        let contract = ObservationContract::prepare().unwrap();
        let prepare = |tail, humidity| {
            PreparedEnvironmentQuantityObservations::prepare(
                RecordingContext {
                    host: &host,
                    boot: &boot,
                    plan: &plan,
                    play: &play,
                },
                &contract,
                1,
                channel("source/t", "channel/t"),
                humidity,
                tail,
            )
        };
        for tail in [0, 17] {
            assert_eq!(
                prepare(tail, channel("source/h", "channel/h")).err(),
                Some(EnvironmentQuantityRefusal::Window)
            );
        }
        assert_eq!(
            prepare(16, channel("source/t", "channel/h")).err(),
            Some(EnvironmentQuantityRefusal::Identity)
        );
    }
    #[test]
    fn oversized_reported_context_refuses_before_reader_preparation() {
        let (host, boot, plan, play) = (
            "h".repeat(129).into(),
            "boot/test".into(),
            "plan/test".into(),
            "play/test".into(),
        );
        let result = PreparedEnvironmentQuantityObservations::prepare(
            RecordingContext {
                host: &host,
                boot: &boot,
                plan: &plan,
                play: &play,
            },
            &ObservationContract::prepare().unwrap(),
            1,
            channel("source/t", "channel/t"),
            channel("source/h", "channel/h"),
            16,
        );
        assert_eq!(result.err(), Some(EnvironmentQuantityRefusal::Identity));
    }
    #[test]
    fn preparation_model_covers_retained_converter_and_generated_output() {
        let (mut prepared, chunk) = fixture(16);
        let contract = ObservationContract::prepare().unwrap();
        let bound =
            PreparedEnvironmentQuantityObservations::preparation_backing_bound(&contract).unwrap();
        assert!(prepared.retained_backing_bytes().unwrap() <= bound);
        prepared.read_next(&chunk).unwrap();
        let snapshot = prepared.snapshot().unwrap();
        let output =
            conduit_data::resolved_quantity_observation_snapshot_backing_bytes(&snapshot).unwrap();
        assert!(
            prepared
                .retained_backing_bytes()
                .unwrap()
                .checked_add(output)
                .unwrap()
                <= bound
        );
    }
}
