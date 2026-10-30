//! Real snapshot content, not a fake durable store or a receipt fixture.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedObservationQuantityForms},
    recording::*,
};
fn chunk(contract: &ObservationContract) -> PreparedRecordingChunk {
    PreparedRecordingChunk::prepare(
        RecordingContext {
            host: &HostId::from("host/sensor"),
            boot: &BootId::from("boot/one"),
            plan: &PlanId::from("plan/exact"),
            play: &ActivePlayId::from("play/exact"),
        },
        contract,
    )
    .unwrap()
}
fn unhex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(core::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn sixty_four_samples_seal_as_bounded_json_without_allocating_or_claiming_commit() {
    let contract = ObservationContract::prepare().unwrap();
    let mut quantities = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = quantities.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    let mut chunk = chunk(&contract);
    let ((), measured) = allocation_probe::observe(|| {
        for sequence in 1..=64 {
            assert_eq!(
                chunk
                    .append(
                        sequence,
                        sequence * 60_000,
                        RecordingDisposition::Available,
                        Some(&observation)
                    )
                    .unwrap(),
                ChunkAppend::Buffered { gap_before: None }
            );
        }
        assert_eq!(
            chunk.append(
                65,
                65 * 60_000,
                RecordingDisposition::Available,
                Some(&observation)
            ),
            Err(RecordingRefusal::Full)
        );
        let first = chunk.seal().unwrap().as_ptr();
        assert!(chunk.seal().unwrap().len() <= MAXIMUM_CHUNK_BYTES);
        assert_eq!(chunk.seal().unwrap().as_ptr(), first);
        assert_eq!(
            chunk.append(65, 65 * 60_000, RecordingDisposition::CaptureTimeout, None),
            Err(RecordingRefusal::Sealed)
        );
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    let json: serde_json::Value = serde_json::from_str(chunk.seal().unwrap()).unwrap();
    assert_eq!(unhex(json["host"].as_str().unwrap()), b"host/sensor");
    assert_eq!(unhex(json["boot"].as_str().unwrap()), b"boot/one");
    assert_eq!(
        json["temperature_form"],
        PreparedObservationQuantityForms::TEMPERATURE_FORM_ID
    );
    assert_eq!(json["disposition_form"], RecordingDispositionForm::IDENTITY);
    let records = unhex(json["records"].as_str().unwrap());
    assert_eq!(records.len(), 64 * 21);
    for (index, record) in records.as_chunks::<21>().0.iter().enumerate() {
        assert_eq!(
            u64::from_le_bytes(record[..8].try_into().unwrap()),
            index as u64 + 1
        );
        assert_eq!(
            RecordingDispositionForm::decode(&record[16..17]).unwrap(),
            RecordingDisposition::Available
        );
        assert_eq!(quantities.unpack(&record[17..]).unwrap(), observation);
    }
    assert!(json.get("committed").is_none());
}
#[test]
fn replay_conflict_order_gaps_clock_and_failure_absence_are_explicit() {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = chunk(&contract);
    assert_eq!(chunk.seal(), Err(RecordingRefusal::Empty));
    assert_eq!(
        chunk.append(0, 0, RecordingDisposition::CaptureTimeout, None),
        Err(RecordingRefusal::InvalidSequence)
    );
    assert_eq!(
        chunk
            .append(10, 100, RecordingDisposition::CaptureTimeout, None)
            .unwrap(),
        ChunkAppend::Buffered { gap_before: None }
    );
    assert_eq!(
        chunk
            .append(10, 100, RecordingDisposition::CaptureTimeout, None)
            .unwrap(),
        ChunkAppend::Duplicate
    );
    assert_eq!(
        chunk.append(10, 100, RecordingDisposition::Checksum, None),
        Err(RecordingRefusal::Conflict)
    );
    assert_eq!(
        chunk.append(9, 100, RecordingDisposition::Checksum, None),
        Err(RecordingRefusal::OutOfOrder)
    );
    assert_eq!(
        chunk.append(11, 99, RecordingDisposition::Checksum, None),
        Err(RecordingRefusal::ClockReversed)
    );
    assert_eq!(
        chunk
            .append(12, 200, RecordingDisposition::Checksum, None)
            .unwrap(),
        ChunkAppend::Buffered {
            gap_before: Some((11, 11))
        }
    );
    assert_eq!(
        chunk.append(13, 300, RecordingDisposition::Available, None),
        Err(RecordingRefusal::MissingObservation)
    );
    assert_eq!(
        chunk.append(13, 300, RecordingDisposition::Checksum, Some(b"bad")),
        Err(RecordingRefusal::UnexpectedObservation)
    );
    assert!(matches!(
        chunk.append(13, 300, RecordingDisposition::Available, Some(b"bad")),
        Err(RecordingRefusal::Observation(_))
    ));
    assert_eq!(chunk.len(), 2);
    let json: serde_json::Value = serde_json::from_str(chunk.seal().unwrap()).unwrap();
    let records = unhex(json["records"].as_str().unwrap());
    for record in records.as_chunks::<21>().0.iter() {
        assert_eq!(&record[17..], &[0; 4]);
    }
}
#[test]
fn context_is_finite_and_is_not_invented_by_the_encoder() {
    let contract = ObservationContract::prepare().unwrap();
    assert!(matches!(
        PreparedRecordingChunk::prepare(
            RecordingContext {
                host: &HostId::from(""),
                boot: &BootId::from("boot"),
                plan: &PlanId::from("plan"),
                play: &ActivePlayId::from("play"),
            },
            &contract
        ),
        Err(RecordingRefusal::ContextExtent)
    ));
    assert!(matches!(
        PreparedRecordingChunk::prepare(
            RecordingContext {
                host: &HostId::from("x".repeat(129)),
                boot: &BootId::from("boot"),
                plan: &PlanId::from("plan"),
                play: &ActivePlayId::from("play"),
            },
            &contract
        ),
        Err(RecordingRefusal::ContextExtent)
    ));
}

#[test]
fn every_authored_absence_and_sequence_exhaustion_remains_explicit() {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = chunk(&contract);
    let states = [
        RecordingDisposition::CaptureTimeout,
        RecordingDisposition::ProviderFailed,
        RecordingDisposition::GeometryRefused,
        RecordingDisposition::ShortFrame,
        RecordingDisposition::MalformedLength,
        RecordingDisposition::Checksum,
        RecordingDisposition::InvalidPayload,
    ];
    for (index, state) in states.iter().enumerate() {
        chunk.append(index as u64 + 1, 1, *state, None).unwrap();
    }
    let json: serde_json::Value = serde_json::from_str(chunk.seal().unwrap()).unwrap();
    let records = unhex(json["records"].as_str().unwrap());
    for (record, state) in records.as_chunks::<21>().0.iter().zip(states) {
        assert_eq!(
            RecordingDispositionForm::decode(&record[16..17]).unwrap(),
            state
        );
        assert_eq!(&record[17..], &[0; 4]);
    }
    let mut last = PreparedRecordingChunk::prepare(
        RecordingContext {
            host: &HostId::from("host"),
            boot: &BootId::from("boot"),
            plan: &PlanId::from("plan"),
            play: &ActivePlayId::from("play"),
        },
        &contract,
    )
    .unwrap();
    last.append(
        u64::MAX,
        u64::MAX,
        RecordingDisposition::CaptureTimeout,
        None,
    )
    .unwrap();
    assert_eq!(
        last.append(
            u64::MAX,
            u64::MAX,
            RecordingDisposition::CaptureTimeout,
            None
        )
        .unwrap(),
        ChunkAppend::Duplicate
    );
    assert_eq!(
        last.append(0, u64::MAX, RecordingDisposition::CaptureTimeout, None),
        Err(RecordingRefusal::InvalidSequence)
    );
}

fn snapshot_context() -> RecordingContext<'static> {
    // Test provenance is supplied, never inferred from content or a reference.
    use std::sync::LazyLock;
    static HOST: LazyLock<HostId> = LazyLock::new(|| "host/sensor".into());
    static BOOT: LazyLock<BootId> = LazyLock::new(|| "boot/one".into());
    static PLAN: LazyLock<PlanId> = LazyLock::new(|| "plan/exact".into());
    static PLAY: LazyLock<ActivePlayId> = LazyLock::new(|| "play/exact".into());
    RecordingContext {
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
#[test]
fn selected_snapshot_extent_is_known_before_samples_and_never_changes_on_partial_flush() {
    use conduit_core::*;
    let contract = ObservationContract::prepare().unwrap();
    let bytes = PreparedRecordingSnapshot::content_bytes(snapshot_context(), &contract, 2).unwrap();
    let mut reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("text/json-utf8@1"),
        access_class: "resource/snapshot@1".into(),
        extent: ResourceExtent {
            bytes: bytes as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    for count in [0, 65, usize::MAX] {
        assert_eq!(
            PreparedRecordingSnapshot::content_bytes(snapshot_context(), &contract, count),
            Err(RecordingSnapshotRefusal::Samples)
        );
    }
    for profile in ["text/plain@1", ""] {
        reference.content_profile = kind_id(profile);
        assert!(
            PreparedRecordingSnapshot::prepare(snapshot_context(), &contract, 2, &reference)
                .is_err()
        );
    }
    reference.content_profile = kind_id("text/json-utf8@1");
    reference.access_class = "resource/other@1".into();
    assert!(matches!(
        PreparedRecordingSnapshot::prepare(snapshot_context(), &contract, 2, &reference),
        Err(RecordingSnapshotRefusal::Extent)
    ));
    reference.access_class = "resource/snapshot@1".into();
    reference.extent.items = Some(2);
    assert!(matches!(
        PreparedRecordingSnapshot::prepare(snapshot_context(), &contract, 2, &reference),
        Err(RecordingSnapshotRefusal::Extent)
    ));
    reference.extent.items = Some(1);
    reference.extent.bytes -= 1;
    assert!(matches!(
        PreparedRecordingSnapshot::prepare(snapshot_context(), &contract, 2, &reference),
        Err(RecordingSnapshotRefusal::Extent)
    ));
    reference.extent.bytes += 1;
    let mut snapshot =
        PreparedRecordingSnapshot::prepare(snapshot_context(), &contract, 2, &reference).unwrap();
    assert_eq!(snapshot.expected_reference(), reference.encode().unwrap());
    let (_, measured) = allocation_probe::observe(|| {
        snapshot
            .append(4, 100, RecordingDisposition::CaptureTimeout, None)
            .unwrap();
        assert_eq!(snapshot.seal(), Err(RecordingSnapshotRefusal::Incomplete));
        assert_eq!(snapshot.len(), 1);
        snapshot
            .append(6, 200, RecordingDisposition::Checksum, None)
            .unwrap();
        assert_eq!(
            snapshot.append(6, 200, RecordingDisposition::Checksum, None),
            Ok(ChunkAppend::Duplicate)
        );
        assert_eq!(
            snapshot.append(6, 201, RecordingDisposition::Checksum, None),
            Err(RecordingSnapshotRefusal::Recording(
                RecordingRefusal::Conflict
            ))
        );
        assert_eq!(
            snapshot.append(7, 300, RecordingDisposition::CaptureTimeout, None),
            Err(RecordingSnapshotRefusal::Recording(RecordingRefusal::Full))
        );
        assert_eq!(snapshot.seal().unwrap().len(), bytes);
        assert_eq!(snapshot.seal().unwrap().len(), bytes);
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
    let mut reader = PreparedRecordingReader::prepare(snapshot_context(), &contract).unwrap();
    reader.read(snapshot.seal().unwrap()).unwrap();
}

#[path = "recording_chunks/catalog.rs"]
mod catalog;

#[path = "recording_chunks/batch.rs"]
mod batch;
