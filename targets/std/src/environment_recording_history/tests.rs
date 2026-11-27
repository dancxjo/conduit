//! Actual original-file capture and frozen worker CPU proof, not recording ACK.
use super::snapshot_fixture::{Directory, Facts};
use super::*;
use crate::{NativeReadContext, NativeSourceWorkerProvider};
use conduit_core::{validate_canonical_structured_value, Quantity, ResourceRetention};
use conduit_dht11_observation::{
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition as D},
    reported_generation::PreparedReportedRecordingGenerationInput as Decoder,
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
            [
                "source/h",
                "Relative humidity",
                "channel/h",
                "Relative humidity",
            ],
        ],
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
pub(super) fn full_json(contract: &ObservationContract) -> String {
    recording(
        contract,
        &(1..=64)
            .map(|n| {
                (
                    2 * n - 1,
                    n * n * 7,
                    if n % 2 == 0 {
                        D::Available
                    } else {
                        D::Checksum
                    },
                )
            })
            .collect::<Vec<_>>(),
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
    if !write {
        facts.binding.content.as_mut().unwrap().contract.retention = ResourceRetention::Boot;
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
fn expected(contract: &ObservationContract) -> [Quantity; 2] {
    let mut forms = PreparedObservationQuantityForms::prepare(contract).unwrap();
    let observation =
        validate_canonical_structured_value(forms.unpack(&[198, 0, 78, 2]).unwrap()).unwrap();
    ["temperature", "humidity"].map(|name| {
        Quantity::decode(
            &observation
                .record_field(name)
                .unwrap()
                .unwrap()
                .value_node()[5..],
        )
        .unwrap()
    })
}
#[test]
fn original_generation_preserves_all_readings_then_emits_without_io_or_allocation() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let original = json(&contract);
    publish(&directory, &original);
    let selected = facts(original.len(), false);
    let mut provider =
        prepare_capture(&contract, &parameters(), selected.capture(&directory)).unwrap();
    assert!(
        provider.prepared_heap_bytes()
            <= conduit_dht11_observation::reported_generation::maximum_canonical_bytes().unwrap()
                as usize
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
    let mut decoded = Decoder::prepare()
        .unwrap()
        .decode(&output[..length], &contract)
        .unwrap();
    assert_eq!(decoded.original_json(), original);
    assert_eq!(decoded.producer(), parameters().producer);
    assert_eq!(decoded.len(), 3);
    assert_eq!(decoded.recovery().chunks, 1);
    assert_eq!(decoded.recovery().samples, 3);
    assert_eq!(decoded.recovery().samples_outside_view, 0);
    assert!(!decoded.recovery().preceding_history_known);
    for (index, names) in parameters().channels.iter().enumerate() {
        let channel = decoded.channel(index).unwrap();
        assert_eq!(
            [
                channel.source_identity,
                channel.source_name,
                channel.channel_identity,
                channel.channel_name
            ],
            *names
        );
    }
    let sample = decoded.sample(1).unwrap();
    assert_eq!(
        (
            sample.sequence,
            sample.sampled_at_boot_ms,
            sample.disposition
        ),
        (4, 7, D::Checksum)
    );
    assert_eq!(sample.gap_before, Some((2, 3)));
    assert!(sample.observation.is_none());
    let sample = decoded.sample(2).unwrap();
    assert_eq!((sample.sequence, sample.sampled_at_boot_ms), (5, 101));
    let observation = validate_canonical_structured_value(sample.observation.unwrap()).unwrap();
    for (name, quantity) in ["temperature", "humidity"]
        .into_iter()
        .zip(expected(&contract))
    {
        assert_eq!(
            Quantity::decode(
                &observation
                    .record_field(name)
                    .unwrap()
                    .unwrap()
                    .value_node()[5..]
            )
            .unwrap(),
            quantity
        );
    }
}
#[test]
fn full_sixty_four_original_samples_keep_all_outcomes_quantities_and_dense_gaps() {
    let statuses = [
        D::Available,
        D::CaptureTimeout,
        D::ProviderFailed,
        D::GeometryRefused,
        D::ShortFrame,
        D::MalformedLength,
        D::Checksum,
        D::InvalidPayload,
    ];
    let events = (0..64)
        .map(|n| (2 * n + 1, n * n * 7, statuses[n as usize % 8]))
        .collect::<Vec<_>>();
    let contract = ObservationContract::prepare().unwrap();
    let original = recording(&contract, &events);
    assert!(original.len() <= 4096);
    let root = Directory::new();
    let directory = root.open();
    publish(&directory, &original);
    let selected = facts(original.len(), false);
    let provider = prepare_capture(&contract, &parameters(), selected.capture(&directory)).unwrap();
    let mut decoded = Decoder::prepare()
        .unwrap()
        .decode(&provider.frozen, &contract)
        .unwrap();
    assert_eq!(decoded.original_json(), original);
    assert_eq!(decoded.len(), 64);
    assert_eq!(decoded.recovery().samples, 64);
    assert_eq!(decoded.recovery().samples_outside_view, 0);
    for (index, event) in events.iter().enumerate() {
        let sample = decoded.sample(index).unwrap();
        assert_eq!(
            (
                sample.sequence,
                sample.sampled_at_boot_ms,
                sample.disposition
            ),
            *event
        );
        assert_eq!(
            sample.gap_before,
            (index > 0).then_some((event.0 - 1, event.0 - 1))
        );
        assert_eq!(sample.observation.is_some(), event.2 == D::Available);
        if let Some(value) = sample.observation {
            let value = validate_canonical_structured_value(value).unwrap();
            for (field, quantity) in ["temperature", "humidity"]
                .into_iter()
                .zip(expected(&contract))
            {
                assert_eq!(
                    Quantity::decode(
                        &value.record_field(field).unwrap().unwrap().value_node()[5..]
                    )
                    .unwrap(),
                    quantity
                );
            }
        }
    }
}
#[test]
fn wrong_reported_context_profile_reference_and_missing_record_refuse() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    let selected = facts(json.len(), false);
    assert!(prepare_capture(&contract, &parameters(), selected.capture(&directory)).is_err());
    publish(&directory, &json);
    let mut wrong = parameters();
    wrong.producer[1] = "boot/foreign";
    assert!(prepare_capture(&contract, &wrong, selected.capture(&directory)).is_err());
    let mut wrong = parameters();
    let mut reference = selected.reference.clone();
    reference.extent.bytes -= 1;
    wrong.reference = Some(reference);
    assert!(prepare_capture(&contract, &wrong, selected.capture(&directory)).is_err());
    let mut invalid = selected.reference.clone();
    invalid.content_profile = "value/bytes".into();
    let mut capture = selected.capture(&directory);
    capture.reference = &invalid;
    assert!(prepare_capture(&contract, &parameters(), capture).is_err());
}
#[test]
fn independent_read_authority_and_bounded_metadata_are_required_before_capture() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    publish(&directory, &json);
    for mode in 0..5 {
        let mut selected = facts(json.len(), false);
        match mode {
            0 => selected.original.boot_id = "boot/foreign".into(),
            1 => selected.original.grant_id = "grant/foreign".into(),
            2 => selected.local.authority_grant = "grant/foreign".into(),
            3 => selected.local.availability = conduit_core::ResourceReferenceAvailability::Lost,
            _ => selected.authority.grant_id = "g".repeat(513).into(),
        }
        assert!(prepare_capture(&contract, &parameters(), selected.capture(&directory)).is_err());
    }
}
#[test]
fn frozen_worker_refuses_invalid_tick_capacity_cancellation_and_expired_deadline() {
    let contract = ObservationContract::prepare().unwrap();
    let root = Directory::new();
    let directory = root.open();
    let json = json(&contract);
    publish(&directory, &json);
    let selected = facts(json.len(), false);
    let mut provider =
        prepare_capture(&contract, &parameters(), selected.capture(&directory)).unwrap();
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
