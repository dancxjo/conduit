use super::allocation_probe;
use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedObservationQuantityForms},
    recording::*,
};
fn context() -> RecordingContext<'static> {
    // Explicit CPU producer identity; no installed Host or durable receipt.
    static HOST: std::sync::LazyLock<HostId> = std::sync::LazyLock::new(|| "sensor/test".into());
    static BOOT: std::sync::LazyLock<BootId> = std::sync::LazyLock::new(|| "boot/test".into());
    static PLAN: std::sync::LazyLock<PlanId> = std::sync::LazyLock::new(|| "plan/test".into());
    static PLAY: std::sync::LazyLock<ActivePlayId> =
        std::sync::LazyLock::new(|| "play/test".into());
    RecordingContext {
        timestamp_basis:
            conduit_environment_runtime::recording::RecordingTimestampBasis::Acquisition,
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
fn chunk(events: &[(u64, u64)]) -> String {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    for &(sequence, time) in events {
        chunk
            .append(sequence, time, RecordingDisposition::CaptureTimeout, None)
            .unwrap();
    }
    chunk.seal().unwrap().to_owned()
}
fn history(maximum_chunks: u16) -> PreparedRecordingHistory {
    PreparedRecordingHistory::prepare(
        context(),
        &ObservationContract::prepare().unwrap(),
        maximum_chunks,
    )
    .unwrap()
}
#[test]
fn ordered_chunks_preserve_quantities_failures_and_boundary_gaps_without_allocations() {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    let first = chunk(&[(7, 0), (8, 15_000)]);
    let mut next = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    next.append(
        11,
        60_000,
        RecordingDisposition::Available,
        Some(&observation),
    )
    .unwrap();
    next.append(13, 90_000, RecordingDisposition::Checksum, None)
        .unwrap();
    let second = next.seal().unwrap();
    let mut history = history(2);
    let ((), measured) = allocation_probe::observe(|| {
        history.read_next(&first).unwrap();
        assert_eq!(history.sample(0).unwrap().gap_before, None);
        history.read_next(second).unwrap();
        let sample = history.sample(0).unwrap();
        assert_eq!(sample.gap_before, Some((9, 10)));
        assert_eq!(sample.observation, Some(observation.as_slice()));
        let sample = history.sample(1).unwrap();
        assert_eq!(sample.gap_before, Some((12, 12)));
        assert_eq!(sample.disposition, RecordingDisposition::Checksum);
        assert!(sample.observation.is_none());
        assert_eq!(
            history.recovered(),
            RecoveredHistory {
                chunks: 2,
                samples: 4,
                first_sequence: Some(7),
                last_sequence: Some(13)
            }
        );
        assert_eq!(
            history.read_next(second),
            Err(RecordingHistoryRefusal::Capacity)
        );
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn boundary_replay_clock_foreign_context_and_corruption_refuse_atomically() {
    let mut history = history(3);
    let first = chunk(&[(1, 10), (2, 20)]);
    history.read_next(&first).unwrap();
    let bads = [
        (first.clone(), RecordingReadRefusal::Sequence),
        (chunk(&[(2, 20), (3, 30)]), RecordingReadRefusal::Sequence),
        (chunk(&[(3, 19)]), RecordingReadRefusal::ClockReversed),
        (
            chunk(&[(3, 30)]).replace("626f6f742f74657374", "626f6f742f6f74686572"),
            RecordingReadRefusal::ContextOrRepresentation,
        ),
        (
            chunk(&[(3, 30)]).replace("\"records\":\"", "\"records\":\"z"),
            RecordingReadRefusal::Extent,
        ),
    ];
    for (json, expected) in bads {
        let before = history.recovered();
        assert_eq!(
            history.read_next(&json),
            Err(RecordingHistoryRefusal::Content(expected))
        );
        assert_eq!(history.recovered(), before);
        assert_eq!(history.loaded_samples(), 2);
        assert_eq!(history.sample(1).unwrap().sequence, 2);
        assert_eq!(history.sample(0).unwrap().gap_before, None);
    }
    // Refusal did not consume a chunk slot or alter the accepted boundary.
    history.read_next(&chunk(&[(3, 20)])).unwrap();
    assert_eq!(history.sample(0).unwrap().gap_before, None);
    history.read_next(&chunk(&[(5, 21)])).unwrap();
    assert_eq!(history.sample(0).unwrap().gap_before, Some((4, 4)));
    assert_eq!(history.recovered().chunks, 3);
}
#[test]
fn admitted_sixty_four_chunk_history_has_no_hidden_eviction_or_growth() {
    let contract = ObservationContract::prepare().unwrap();
    let chunks: Vec<_> = (0..64)
        .map(|i| {
            let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
            for j in 1..=64 {
                let seq = i * 64 + j;
                chunk
                    .append(
                        seq,
                        seq * 15_000,
                        RecordingDisposition::ProviderFailed,
                        None,
                    )
                    .unwrap();
            }
            chunk.seal().unwrap().to_owned()
        })
        .collect();
    let mut history = history(64);
    let ((), measured) = allocation_probe::observe(|| {
        for json in &chunks {
            history.read_next(json).unwrap();
            assert_eq!(history.sample(0).unwrap().gap_before, None);
        }
        assert_eq!(
            history.recovered(),
            RecoveredHistory {
                chunks: 64,
                samples: 4096,
                first_sequence: Some(1),
                last_sequence: Some(4096)
            }
        );
        assert_eq!(
            history.read_next(&chunks[0]),
            Err(RecordingHistoryRefusal::Capacity)
        );
        assert_eq!(history.sample(63).unwrap().sequence, 4096);
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
#[test]
fn empty_admission_refuses_and_standalone_reader_does_not_inherit_history() {
    assert!(matches!(
        PreparedRecordingHistory::prepare(context(), &ObservationContract::prepare().unwrap(), 0),
        Err(RecordingHistoryRefusal::ZeroChunks)
    ));
    let mut reader =
        PreparedRecordingReader::prepare(context(), &ObservationContract::prepare().unwrap())
            .unwrap();
    reader.read(&chunk(&[(1, 10)])).unwrap();
    reader.read(&chunk(&[(7, 20)])).unwrap();
    assert_eq!(reader.sample(0).unwrap().gap_before, None);
}
