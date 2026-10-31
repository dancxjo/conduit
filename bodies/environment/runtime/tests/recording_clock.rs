#![cfg(feature = "face-projection")]
//! Time meaning survives encoding/recovery; this is not origin authentication.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{ActivePlayId, BootId, HostId, PlanId, Quantity, Unit};
use conduit_environment_runtime::{
    observation::ObservationContract, observed_record::ObservedRecord, recording::*,
};
fn context(basis: RecordingTimestampBasis) -> RecordingContext<'static> {
    use std::sync::LazyLock;
    static HOST: LazyLock<HostId> = LazyLock::new(|| "fixture/sensor".into());
    static BOOT: LazyLock<BootId> = LazyLock::new(|| "fixture/boot".into());
    static PLAN: LazyLock<PlanId> = LazyLock::new(|| "fixture/plan".into());
    static PLAY: LazyLock<ActivePlayId> = LazyLock::new(|| "fixture/play".into());
    RecordingContext {
        timestamp_basis: basis,
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
fn record(sequence: u64, at_boot_ms: u64) -> ObservedRecord {
    ObservedRecord {
        sequence,
        observed_at_boot_ms: at_boot_ms,
        disposition: RecordingDisposition::Available,
        quantities: Some((
            Quantity::new(21, Unit::Celsius),
            Quantity::new(61, Unit::Percent),
        )),
    }
}
#[test]
fn observed_records_keep_exact_bytes_and_clock_without_allocating() {
    let contract = ObservationContract::prepare().unwrap();
    let basis = RecordingTimestampBasis::ProducerObservation;
    let mut chunk = PreparedRecordingChunk::prepare(context(basis), &contract).unwrap();
    let mut reader = PreparedRecordingReader::prepare(context(basis), &contract).unwrap();
    let ((), allocation) = allocation_probe::observe(|| {
        for sequence in 1..=64 {
            chunk
                .append_observed(&record(sequence, sequence * 100))
                .unwrap();
        }
        let encoded = chunk.seal().unwrap();
        assert!(encoded.contains("\"clock\":\"producer-observation-boot-ms\""));
        reader.read(encoded).unwrap();
        for index in 0..64 {
            let sample = reader.sample(index).unwrap();
            assert_eq!(sample.sequence, index as u64 + 1);
            assert_eq!(sample.at_boot_ms, sample.sequence * 100);
            assert_eq!(sample.timestamp_basis, basis);
        }
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let json: serde_json::Value = serde_json::from_str(chunk.seal().unwrap()).unwrap();
    let encoded = json["records"].as_str().unwrap();
    for index in 0..64 {
        let bytes = record(index + 1, (index + 1) * 100).encode().unwrap();
        let expected: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            &encoded[index as usize * 42..(index as usize + 1) * 42],
            expected
        );
    }
}
#[test]
fn acquisition_cannot_accept_observed_records_or_relabel_observed_content() {
    let contract = ObservationContract::prepare().unwrap();
    let mut acquired =
        PreparedRecordingChunk::prepare(context(RecordingTimestampBasis::Acquisition), &contract)
            .unwrap();
    assert_eq!(
        acquired.append_observed(&record(1, 100)),
        Err(RecordingRefusal::ClockBasis)
    );
    assert!(acquired.is_empty());
    acquired
        .append(1, 90, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    let legacy = acquired.seal().unwrap().to_owned();
    assert!(legacy.contains("\"clock\":\"producer-boot-ms\""));
    let mut reader =
        PreparedRecordingReader::prepare(context(RecordingTimestampBasis::Acquisition), &contract)
            .unwrap();
    reader.read(&legacy).unwrap();
    let mut observed = PreparedRecordingChunk::prepare(
        context(RecordingTimestampBasis::ProducerObservation),
        &contract,
    )
    .unwrap();
    observed.append_observed(&record(2, 100)).unwrap();
    assert_eq!(
        reader.read(observed.seal().unwrap()),
        Err(RecordingReadRefusal::ContextOrRepresentation)
    );
    assert_eq!(reader.sample(0).unwrap().at_boot_ms, 90);
    assert_eq!(reader.sample(0).unwrap().sequence, 1);
    let mut observed_reader = PreparedRecordingReader::prepare(
        context(RecordingTimestampBasis::ProducerObservation),
        &contract,
    )
    .unwrap();
    assert_eq!(
        observed_reader.read(&legacy),
        Err(RecordingReadRefusal::ContextOrRepresentation)
    );
    assert!(observed_reader.is_empty());
}
#[test]
fn observed_failures_conflicts_and_reversed_time_preserve_history() {
    let contract = ObservationContract::prepare().unwrap();
    let basis = RecordingTimestampBasis::ProducerObservation;
    let mut chunk = PreparedRecordingChunk::prepare(context(basis), &contract).unwrap();
    chunk.append_observed(&record(1, 100)).unwrap();
    assert_eq!(
        chunk.append_observed(&record(1, 100)),
        Ok(ChunkAppend::Duplicate)
    );
    assert_eq!(
        chunk.append_observed(&record(1, 101)),
        Err(RecordingRefusal::Conflict)
    );
    assert_eq!(
        chunk.append_observed(&record(2, 99)),
        Err(RecordingRefusal::ClockReversed)
    );
    let mut failure = record(2, 110);
    failure.disposition = RecordingDisposition::ProviderFailed;
    assert!(chunk.append_observed(&failure).is_err()); // quantities on failure
    assert_eq!(chunk.len(), 1);
    failure.quantities = None;
    chunk.append_observed(&failure).unwrap();
    let mut history = PreparedRecordingHistory::prepare(context(basis), &contract, 2).unwrap();
    history.read_next(chunk.seal().unwrap()).unwrap();
    assert_eq!(history.sample(1).unwrap().observation, None);
    let before = history.recovered();
    let mut next = PreparedRecordingChunk::prepare(context(basis), &contract).unwrap();
    next.append_observed(&record(3, 109)).unwrap();
    assert_eq!(
        history.read_next(next.seal().unwrap()),
        Err(RecordingHistoryRefusal::Content(
            RecordingReadRefusal::ClockReversed
        ))
    );
    assert_eq!(history.recovered(), before);
    assert_eq!(history.sample(1).unwrap().at_boot_ms, 110);
}

#[test]
fn fixed_snapshot_extent_does_not_grow_to_fit_another_clock_profile() {
    use conduit_core as c;
    let contract = ObservationContract::prepare().unwrap();
    let acquired_bytes = PreparedRecordingSnapshot::content_bytes(
        context(RecordingTimestampBasis::Acquisition),
        &contract,
        1,
    )
    .unwrap();
    let basis = RecordingTimestampBasis::ProducerObservation;
    let observed_bytes =
        PreparedRecordingSnapshot::content_bytes(context(basis), &contract, 1).unwrap();
    assert!(observed_bytes > acquired_bytes);
    let mut reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([3; 32]),
        content_profile: c::kind_id("text/json-utf8@1"),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: acquired_bytes as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([4; 32]),
            expires_at: None,
        },
    };
    assert!(matches!(
        PreparedRecordingSnapshot::prepare(context(basis), &contract, 1, &reference),
        Err(RecordingSnapshotRefusal::Extent)
    ));
    // Fixture selection of a different exact extent; production Root must admit it.
    reference.extent.bytes = observed_bytes as u64;
    let mut snapshot =
        PreparedRecordingSnapshot::prepare(context(basis), &contract, 1, &reference).unwrap();
    snapshot.append_observed(&record(1, 99)).unwrap();
    assert_eq!(snapshot.seal().unwrap().len(), observed_bytes);
    assert_eq!(
        snapshot.append_observed(&record(2, 100)),
        Err(RecordingSnapshotRefusal::Recording(
            RecordingRefusal::Sealed
        ))
    );
}
