//! Actual file-record capture and pure frozen-worker CPU proof, not installed
//! admission, durable publication ACK, restart, or execution of a live sensor.
use super::*;
use crate::environment_recording_history::snapshot_fixture::{Directory, Facts};
use crate::{NativeReadContext, NativeSourceWorkerProvider};
use conduit_core::{validate_canonical_structured_value, Quantity, ResourceRetention};
use conduit_data::{PreparedQuantityObservationSnapshotInput, QuantityObservationOutcome};
use conduit_dht11_observation::{
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition as D},
    ObservationContract, PreparedObservationQuantityForms,
};
use conduit_linux_snapshot_store::{PreparedFileSnapshot, SnapshotDirectory};
use std::{
    fs,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn parameters() -> CheckedEnvironmentRecordingHistory<'static> {
    CheckedEnvironmentRecordingHistory {
        source_pool: "history/source",
        memory_pool: "history/memory",
        directory: None,
        reference: None,
        producer: [
            "host/reported",
            "boot/reported",
            "plan/reported",
            "play/reported",
        ],
        channels: [
            ["source/t", "Temperature", "channel/t", "Temperature"],
            ["source/h", "Humidity", "channel/h", "Relative humidity"],
        ],
        tail: 2,
    }
}
pub(super) fn json(contract: &ObservationContract) -> String {
    recording(
        contract,
        &[
            (1, 0, D::Available),
            (4, 7, D::Checksum),
            (5, 101, D::Available),
        ],
    )
}
fn recording(contract: &ObservationContract, events: &[(u64, u64, D)]) -> String {
    let (host, boot, plan, play) = (
        "host/reported".into(),
        "boot/reported".into(),
        "plan/reported".into(),
        "play/reported".into(),
    );
    let mut chunk = PreparedRecordingChunk::prepare(
        RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        },
        contract,
    )
    .unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    for &(sequence, time, disposition) in events {
        chunk
            .append(
                sequence,
                time,
                disposition,
                (disposition == D::Available).then_some(observation),
            )
            .unwrap();
    }
    chunk.seal().unwrap().into()
}
pub(super) fn facts(bytes: usize, write: bool) -> Facts {
    let mut facts = Facts::new(1, write, bytes);
    facts.reference.content_profile = "text/json-utf8@1".into();
    facts.local.content_profile = "text/json-utf8@1".into();
    let content = facts.binding.content.as_mut().unwrap();
    content.contract.content_profile = "text/json-utf8@1".into();
    if !write {
        content.contract.retention = ResourceRetention::Boot;
    }
    facts
}
pub(super) fn publish(directory: &SnapshotDirectory, json: &str) {
    let facts = facts(json.len(), true);
    PreparedFileSnapshot::prepare_selected(
        directory,
        facts.selected(),
        &facts.reference,
        &facts.original,
        &facts.local,
    )
    .unwrap()
    .publish(&facts.authority, json.as_bytes())
    .unwrap();
}

#[test]
fn actual_recording_becomes_two_exact_channels_then_reads_without_io_or_allocation() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    publish(&directory, &json);
    let facts = facts(json.len(), false);
    let mut provider =
        prepare_capture(&contract, &parameters(), facts.capture(&directory)).unwrap();
    assert!(
        provider.prepared_heap_bytes()
            <= conduit_data::resolved_quantity_observation_snapshot_byte_bound().unwrap() as usize
    );
    assert!(provider_preparation_bytes(&contract).unwrap() > provider.prepared_heap_bytes());
    drop(directory);
    fs::remove_dir_all(&root.0).unwrap();
    let cancelled = AtomicBool::new(false);
    let context = NativeReadContext::for_test(&cancelled, Instant::now() + Duration::from_secs(2));
    let mut output = [0; 16 * 1024];
    let tick = conduit_time::encode_tick(1);
    let probe = crate::allocation_probe::begin();
    let length = provider.read(&tick, &mut output, &context).unwrap();
    assert_eq!(probe.finish_bytes(), 0);
    let snapshot = PreparedQuantityObservationSnapshotInput::prepare()
        .unwrap()
        .decode(&output[..length])
        .unwrap();
    assert_eq!(snapshot.batches.len(), 2);
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation =
        validate_canonical_structured_value(forms.unpack(&[198, 0, 78, 2]).unwrap()).unwrap();
    for ((batch, field), names) in snapshot
        .batches
        .iter()
        .zip(["temperature", "humidity"])
        .zip(parameters().channels)
    {
        assert_eq!(batch.source_identity, names[0]);
        assert_eq!(batch.source_name, names[1]);
        assert_eq!(batch.channel_identity, names[2]);
        assert_eq!(batch.channel_name, names[3]);
        assert_eq!(batch.producer_host, "host/reported");
        assert_eq!(batch.producer_boot, "boot/reported");
        assert_eq!(batch.producer_plan, "plan/reported");
        assert_eq!(batch.producer_play, "play/reported");
        assert_eq!(batch.samples_outside_view, 1);
        let samples = batch.observations.iter().collect::<Vec<_>>();
        assert_eq!(samples.len(), 2);
        assert_eq!((samples[0].sequence, samples[0].sampled_at_boot_ms), (4, 7));
        let gap = samples[0].gap_before.as_ref().unwrap();
        assert_eq!((gap.first_sequence, gap.last_sequence), (2, 3));
        assert_eq!(
            samples[0].outcome,
            QuantityObservationOutcome::Unavailable("checksum".into())
        );
        assert_eq!(
            (samples[1].sequence, samples[1].sampled_at_boot_ms),
            (5, 101)
        );
        let leaf = observation.record_field(field).unwrap().unwrap();
        let expected = Quantity::decode(&leaf.value_node()[5..]).unwrap();
        assert_eq!(
            samples[1].outcome,
            QuantityObservationOutcome::Available(Box::new(expected))
        );
    }
}

#[test]
fn wrong_reported_context_profile_configured_reference_and_missing_record_refuse() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    let facts = facts(json.len(), false);
    assert!(prepare_capture(&contract, &parameters(), facts.capture(&directory)).is_err());
    publish(&directory, &json);
    let mut wrong = parameters();
    wrong.producer[1] = "boot/foreign";
    assert!(prepare_capture(&contract, &wrong, facts.capture(&directory)).is_err());
    let mut wrong = parameters();
    let mut reference = facts.reference.clone();
    reference.extent.bytes -= 1;
    wrong.reference = Some(reference);
    assert!(prepare_capture(&contract, &wrong, facts.capture(&directory)).is_err());
    let mut invalid = facts.reference.clone();
    invalid.content_profile = "value/bytes".into();
    let mut capture = facts.capture(&directory);
    capture.reference = &invalid;
    assert!(prepare_capture(&contract, &parameters(), capture).is_err());
}

fn outcomes() -> [(D, Option<&'static str>); 8] {
    [
        (D::Available, None),
        (D::CaptureTimeout, Some("capture-timeout")),
        (D::ProviderFailed, Some("provider-failed")),
        (D::GeometryRefused, Some("geometry-refused")),
        (D::ShortFrame, Some("short-frame")),
        (D::MalformedLength, Some("malformed-length")),
        (D::Checksum, Some("checksum")),
        (D::InvalidPayload, Some("invalid-payload")),
    ]
}

#[test]
fn explicit_eight_event_view_preserves_all_outcomes_and_dense_sixteen_refuses_extent() {
    let contract = ObservationContract::prepare().unwrap();
    for dense in [false, true] {
        let events = (0..20u64)
            .map(|index| {
                let sequence = if dense {
                    2 * index + 1
                } else {
                    index + 1 + u64::from(index >= 15) * 2
                };
                (
                    sequence,
                    index * index * 7,
                    outcomes()[index as usize % 8].0,
                )
            })
            .collect::<Vec<_>>();
        let json = recording(&contract, &events);
        assert!(json.len() <= 4096);
        let root = Directory::new();
        let directory = root.open();
        publish(&directory, &json);
        let facts = facts(json.len(), false);
        let mut parameters = parameters();
        parameters.tail = if dense { 16 } else { 8 };
        if dense {
            for _ in 0..2 {
                let error = prepare_capture(&contract, &parameters, facts.capture(&directory))
                    .err()
                    .expect("requested sixteen-event view must not shrink to fit");
                assert!(error.contains("Extent"), "{error}");
            }
            continue;
        }
        let provider = prepare_capture(&contract, &parameters, facts.capture(&directory)).unwrap();
        let snapshot = PreparedQuantityObservationSnapshotInput::prepare()
            .unwrap()
            .decode(&provider.frozen)
            .unwrap();
        for batch in snapshot.batches.iter() {
            assert_eq!(batch.samples_outside_view, 12);
            assert_eq!(batch.observations.len(), 8);
            for (sample, (index, event)) in batch
                .observations
                .iter()
                .zip(events.iter().enumerate().skip(12))
            {
                assert_eq!(
                    (sample.sequence, sample.sampled_at_boot_ms),
                    (event.0, event.1)
                );
                assert_eq!(
                    sample
                        .gap_before
                        .as_ref()
                        .map(|gap| (gap.first_sequence, gap.last_sequence)),
                    (index == 15).then_some((16, 17))
                );
                match outcomes()[index % 8].1 {
                    Some(reason) => assert_eq!(
                        sample.outcome,
                        QuantityObservationOutcome::Unavailable(reason.into())
                    ),
                    None => assert!(matches!(
                        sample.outcome,
                        QuantityObservationOutcome::Available(_)
                    )),
                }
            }
        }
    }
}

#[test]
fn independent_read_authority_and_bounded_metadata_are_required_before_capture() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    publish(&directory, &json);
    for mode in 0..5 {
        let mut facts = facts(json.len(), false);
        match mode {
            0 => facts.original.boot_id = "boot/foreign".into(),
            1 => facts.original.grant_id = "grant/foreign".into(),
            2 => facts.local.authority_grant = "grant/foreign".into(),
            3 => facts.local.availability = conduit_core::ResourceReferenceAvailability::Lost,
            _ => facts.authority.grant_id = "g".repeat(513).into(),
        }
        assert!(prepare_capture(&contract, &parameters(), facts.capture(&directory)).is_err());
    }
}

#[test]
fn frozen_worker_refuses_invalid_tick_capacity_cancellation_and_expired_deadline() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    publish(&directory, &json);
    let facts = facts(json.len(), false);
    let mut provider =
        prepare_capture(&contract, &parameters(), facts.capture(&directory)).unwrap();
    let cancelled = AtomicBool::new(false);
    let live = NativeReadContext::for_test(&cancelled, Instant::now() + Duration::from_secs(2));
    let tick = conduit_time::encode_tick(1);
    let mut output = [0; 16 * 1024];
    assert!(matches!(
        provider.read(&[], &mut output, &live),
        Err(crate::WorkerFailure::Provider)
    ));
    assert!(matches!(
        provider.read(&tick, &mut [], &live),
        Err(crate::WorkerFailure::InvalidOutput)
    ));
    let expired = NativeReadContext::for_test(&cancelled, Instant::now());
    assert!(matches!(
        provider.read(&tick, &mut output, &expired),
        Err(crate::WorkerFailure::TimedOut)
    ));
    cancelled.store(true, std::sync::atomic::Ordering::Release);
    assert!(matches!(
        provider.read(&tick, &mut output, &live),
        Err(crate::WorkerFailure::Cancelled)
    ));
}
