#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedObservationQuantityForms},
    recording::*,
};
fn prepared() -> (PreparedRecordingChunk, PreparedRecordingReader, Vec<u8>) {
    let contract = ObservationContract::prepare().unwrap();
    let host = HostId::from("sensor/α");
    let boot = BootId::from("boot/actual");
    let plan = PlanId::from("plan/exact");
    let play = ActivePlayId::from("play/exact");
    let context = || RecordingContext {
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    (
        PreparedRecordingChunk::prepare(context(), &contract).unwrap(),
        PreparedRecordingReader::prepare(context(), &contract).unwrap(),
        observation,
    )
}
fn change(json: &str, record: usize, byte: usize, value: u8) -> String {
    let offset = json.find("\"records\":\"").unwrap()
        + "\"records\":\"".len()
        + (record * SAMPLE_BYTES + byte) * 2;
    let mut result = json.to_string();
    result.replace_range(offset..offset + 2, &format!("{value:02x}"));
    result
}
#[test]
fn recovered_quantities_failures_and_gaps_require_no_play_allocations() {
    let (mut chunk, mut reader, observation) = prepared();
    let statuses = [
        RecordingDisposition::Available,
        RecordingDisposition::CaptureTimeout,
        RecordingDisposition::ProviderFailed,
        RecordingDisposition::GeometryRefused,
        RecordingDisposition::ShortFrame,
        RecordingDisposition::MalformedLength,
        RecordingDisposition::Checksum,
        RecordingDisposition::InvalidPayload,
    ];
    for (i, status) in statuses.into_iter().enumerate() {
        chunk
            .append(
                i as u64 * 2 + 7,
                i as u64 * 60_000,
                status,
                (i == 0).then_some(observation.as_slice()),
            )
            .unwrap();
    }
    let json = chunk.seal().unwrap();
    let ((), measured) = allocation_probe::observe(|| {
        reader.read(json).unwrap();
        assert_eq!(reader.len(), 8);
        for (i, status) in statuses.into_iter().enumerate() {
            let sample = reader.sample(i).unwrap();
            assert_eq!(sample.disposition, status);
            assert_eq!(sample.sequence, i as u64 * 2 + 7);
            assert_eq!(sample.sampled_at_boot_ms, i as u64 * 60_000);
            assert_eq!(
                sample.observation,
                (i == 0).then_some(observation.as_slice())
            );
            assert_eq!(
                sample.gap_before,
                if i == 0 {
                    None
                } else {
                    Some((i as u64 * 2 + 6, i as u64 * 2 + 6))
                }
            );
        }
        assert!(reader.sample(8).is_none());
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn corrupted_records_refuse_atomically_without_replacing_prior_recovery() {
    let (mut chunk, mut reader, _) = prepared();
    chunk
        .append(1, 1, RecordingDisposition::Checksum, None)
        .unwrap();
    chunk
        .append(2, 2, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    let json = chunk.seal().unwrap();
    reader.read(json).unwrap();
    for (bad, expected) in [
        (change(json, 0, 16, 255), RecordingReadRefusal::Disposition),
        (
            change(json, 1, 17, 1),
            RecordingReadRefusal::NoncanonicalFailure,
        ),
        (change(json, 1, 0, 1), RecordingReadRefusal::Sequence),
        (change(json, 1, 0, 0), RecordingReadRefusal::Sequence),
        (change(json, 1, 8, 0), RecordingReadRefusal::ClockReversed),
    ] {
        assert_eq!(reader.read(&bad), Err(expected));
        assert_eq!(reader.len(), 2);
        assert_eq!(reader.sample(1).unwrap().sequence, 2);
    }
}
#[test]
fn wrong_identity_form_clock_and_noncanonical_json_cannot_be_reinterpreted() {
    let (mut chunk, mut reader, _) = prepared();
    chunk
        .append(1, 0, RecordingDisposition::Checksum, None)
        .unwrap();
    let json = chunk.seal().unwrap();
    for field in [
        "host",
        "boot",
        "plan",
        "play",
        "observation_type",
        "temperature_form",
        "humidity_form",
        "disposition_form",
        "clock",
    ] {
        let key = format!("\"{field}\":\"");
        let offset = json.find(&key).unwrap() + key.len();
        let mut bad = json.to_string();
        bad.replace_range(offset..offset + 1, "X");
        assert_eq!(
            reader.read(&bad),
            Err(RecordingReadRefusal::ContextOrRepresentation),
            "{field}"
        );
        assert!(reader.is_empty());
    }
    assert_eq!(
        reader.read(&format!("{json}\n")),
        Err(RecordingReadRefusal::ContextOrRepresentation)
    );
    let mut bad = change(json, 0, 0, 171);
    let start = bad.find("\"records\":\"").unwrap() + "\"records\":\"".len();
    bad.replace_range(start..start + 2, "AB");
    assert_eq!(reader.read(&bad), Err(RecordingReadRefusal::Hex));
    // Retain canonical prefix ordering rather than serde's object key ordering.
    let offset = json.find("\"records\":\"").unwrap() + "\"records\":\"".len();
    assert_eq!(
        reader.read(&format!("{}\"}}", &json[..offset])),
        Err(RecordingReadRefusal::Extent)
    );
    assert_eq!(
        reader.read(&" ".repeat(MAXIMUM_CHUNK_BYTES + 1)),
        Err(RecordingReadRefusal::Extent)
    );
}

#[test]
fn full_chunk_recovers_but_truncation_or_sixty_fifth_record_refuses() {
    let (mut chunk, mut reader, observation) = prepared();
    for sequence in 1..=64 {
        chunk
            .append(
                sequence,
                sequence,
                RecordingDisposition::Available,
                Some(&observation),
            )
            .unwrap();
    }
    let json = chunk.seal().unwrap();
    reader.read(json).unwrap();
    assert_eq!(reader.len(), 64);
    assert_eq!(
        reader.sample(63).unwrap().observation,
        Some(observation.as_slice())
    );
    let start = json.find("\"records\":\"").unwrap() + "\"records\":\"".len();
    let end = json.len() - 2;
    let mut oversized = json.to_string();
    oversized.insert_str(end, &json[start..start + SAMPLE_BYTES * 2]);
    assert_eq!(reader.read(&oversized), Err(RecordingReadRefusal::Extent));
    let mut truncated = json.to_string();
    truncated.remove(end - 1);
    assert_eq!(reader.read(&truncated), Err(RecordingReadRefusal::Extent));
    assert_eq!(reader.len(), 64);
    assert_eq!(reader.sample(63).unwrap().sequence, 64);
}

#[path = "recording_recovery/history.rs"]
mod history;

#[path = "recording_recovery/back.rs"]
mod back;

#[path = "recording_recovery/installation.rs"]
mod installation;

#[path = "recording_recovery/reference_source.rs"]
mod reference_source;
