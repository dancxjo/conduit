//! Content/reference conformance only; no file, authority or commit fixture.
use super::*;
use conduit_core::{
    ActivePlayId, BootId, HostId, PlanId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity,
};
struct Producer {
    host: HostId,
    boot: BootId,
    plan: PlanId,
    play: ActivePlayId,
}
impl Producer {
    fn new() -> Self {
        Self {
            host: "host/sensor".into(),
            boot: "boot/sensor".into(),
            plan: "plan/sensor".into(),
            play: "play/sensor".into(),
        }
    }
    fn context(&self) -> RecordingContext<'_> {
        RecordingContext {
            host: &self.host,
            boot: &self.boot,
            plan: &self.plan,
            play: &self.play,
        }
    }
}
fn reference(bytes: usize) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("value/text"),
        access_class: conduit_data::DATA_GENERATION_ACCESS_CLASS.into(),
        extent: ResourceExtent {
            bytes: bytes as u64,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
}
#[test]
fn original_coordinates_and_exact_reference_survive_text_round_trip() {
    let producer = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let bytes = PreparedRecordingText::content_bytes(producer.context(), &contract, 2).unwrap();
    let selected = reference(bytes);
    let mut text =
        PreparedRecordingText::prepare(producer.context(), &contract, 2, &selected).unwrap();
    assert!(text.is_empty());
    assert_eq!(text.expected_reference(), selected.encode().unwrap());
    text.append(17, 600_000, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    assert_eq!(text.seal(), Err(RecordingTextRefusal::Incomplete));
    assert_eq!(text.len(), 1);
    text.append(19, 720_000, RecordingDisposition::Checksum, None)
        .unwrap();
    assert_eq!(
        text.append(19, 720_000, RecordingDisposition::Checksum, None),
        Ok(ChunkAppend::Duplicate)
    );
    assert_eq!(
        text.append(19, 720_001, RecordingDisposition::Checksum, None),
        Err(RecordingTextRefusal::Recording(RecordingRefusal::Conflict))
    );
    assert_eq!(
        text.append(20, 780_000, RecordingDisposition::Checksum, None),
        Err(RecordingTextRefusal::Recording(RecordingRefusal::Full))
    );
    let backing = text.retained_backing_bytes();
    let content = text.seal().unwrap();
    assert_eq!(content.len(), bytes);
    let pointer = content.as_ptr();
    let mut reader = PreparedRecordingReader::prepare(producer.context(), &contract).unwrap();
    reader.read(content).unwrap();
    let first = reader.sample(0).unwrap();
    assert_eq!((first.sequence, first.sampled_at_boot_ms), (17, 600_000));
    assert_eq!(first.disposition, RecordingDisposition::CaptureTimeout);
    assert!(first.observation.is_none());
    let second = reader.sample(1).unwrap();
    assert_eq!((second.sequence, second.sampled_at_boot_ms), (19, 720_000));
    assert_eq!(second.gap_before, Some((18, 18)));
    assert_eq!(text.seal().unwrap().as_ptr(), pointer);
    assert_eq!(text.retained_backing_bytes(), backing);
    assert_eq!(
        text.append(19, 720_000, RecordingDisposition::Checksum, None),
        Err(RecordingTextRefusal::Recording(RecordingRefusal::Sealed))
    );
    let mut foreign = Producer::new();
    foreign.boot = "boot/foreign".into();
    let mut reader = PreparedRecordingReader::prepare(foreign.context(), &contract).unwrap();
    assert_eq!(
        reader.read(text.seal().unwrap()),
        Err(RecordingReadRefusal::ContextOrRepresentation)
    );
}
#[test]
fn data_reference_contract_and_exact_extent_are_not_relaxed_for_recording() {
    let producer = Producer::new();
    let contract = ObservationContract::prepare().unwrap();
    let bytes = PreparedRecordingText::content_bytes(producer.context(), &contract, 1).unwrap();
    for invalid in 0..5 {
        let mut selected = reference(bytes);
        match invalid {
            0 => selected.access_class = "resource/snapshot@1".into(),
            1 => selected.content_profile = kind_id("text/json-utf8@1"),
            2 => selected.extent.items = Some(1),
            3 => {
                selected.lifetime.expires_at = Some(conduit_core::TemporalInstant {
                    ticks: 100,
                    scale: conduit_core::TemporalScale::Milliseconds,
                    clock_basis: "clock/test@1".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                })
            }
            4 => selected.extent.bytes -= 1,
            _ => unreachable!(),
        }
        let result = PreparedRecordingText::prepare(producer.context(), &contract, 1, &selected);
        assert!(if invalid == 4 {
            matches!(result, Err(RecordingTextRefusal::Extent))
        } else {
            matches!(result, Err(RecordingTextRefusal::Reference(_)))
        });
    }
    for count in [0, MAXIMUM_CHUNK_SAMPLES + 1, usize::MAX] {
        assert_eq!(
            PreparedRecordingText::content_bytes(producer.context(), &contract, count),
            Err(RecordingTextRefusal::Samples)
        );
        assert!(matches!(
            PreparedRecordingText::prepare(producer.context(), &contract, count, &reference(bytes)),
            Err(RecordingTextRefusal::Samples)
        ));
    }
}
#[test]
fn invalid_producer_context_cannot_be_substituted_by_a_valid_reference() {
    let mut producer = Producer::new();
    producer.host = alloc::string::String::new().into();
    let contract = ObservationContract::prepare().unwrap();
    assert_eq!(
        PreparedRecordingText::content_bytes(producer.context(), &contract, 1),
        Err(RecordingTextRefusal::Recording(
            RecordingRefusal::ContextExtent
        ))
    );
    assert!(matches!(
        PreparedRecordingText::prepare(producer.context(), &contract, 1, &reference(100)),
        Err(RecordingTextRefusal::Recording(
            RecordingRefusal::ContextExtent
        ))
    ));
}
