extern crate std;
use super::*;
use std::{sync::LazyLock, vec::Vec};
mod allocation_probe {
    use super::std;
    use std::thread_local;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../architecture/plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn now() -> u64 {
    100
}
fn context() -> LogContext<'static> {
    static HOST: LazyLock<HostId> = LazyLock::new(|| "host/log".into());
    static BOOT: LazyLock<BootId> = LazyLock::new(|| "boot/log".into());
    static PLAN: LazyLock<PlanId> = LazyLock::new(|| "plan/log".into());
    static PLAY: LazyLock<ActivePlayId> = LazyLock::new(|| "play/log".into());
    LogContext {
        body: "body/log",
        producer: RecordingContext {
            host: &HOST,
            boot: &BOOT,
            plan: &PLAN,
            play: &PLAY,
        },
        stream: [7; 32],
    }
}
fn stage(
    back: &mut RecordingLogBack,
    bytes: &[u8],
    output: Option<u32>,
    fuel: u16,
) -> (StepOutcome, StepIo<1>) {
    let value = conduit_kernel::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    let mut io = StepIo::test_frame([Some(value)], [false], [output], None, fuel);
    let result = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (result, io)
}
fn commit(back: &mut RecordingLogBack) {
    <RecordingLogBack as StepBack<1>>::step_committed(back)
}
fn frame(back: &RecordingLogBack) -> &[u8] {
    <RecordingLogBack as StepBack<1>>::prepared_output(back, PortId(0)).unwrap()
}
#[test]
fn committed_context_precedes_exact_sample_bytes_and_reader_flags_gaps() {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let canonical = forms.unpack(&[0; 4]).unwrap().to_vec();
    let mut back = RecordingLogBack::prepare(context(), &contract, 17, 2, now).unwrap();
    let mut reader = PreparedLogReader::prepare(context(), &contract, 17).unwrap();
    let parts = back.context.parts();
    let mut frames = Vec::new();
    for ordinal in 0..parts {
        let (result, io) = stage(&mut back, &canonical, Some(200), 16);
        assert_eq!(result, StepOutcome::Progress);
        assert!(!io.test_consumed(PortId(0)));
        let first = frame(&back).to_vec();
        assert_eq!(stage(&mut back, &canonical, None, 16).0, StepOutcome::Await);
        commit(&mut back);
        assert_eq!(back.ordinal, ordinal);
        assert_eq!(
            stage(&mut back, &canonical, Some(200), 16).0,
            StepOutcome::Progress
        );
        assert_eq!(frame(&back), first);
        commit(&mut back);
        frames.push(first);
    }
    assert_eq!(back.next_sequence(), 17);
    let (result, io) = stage(&mut back, &canonical, Some(200), 16);
    assert_eq!(result, StepOutcome::Progress);
    assert!(io.test_consumed(PortId(0)));
    let sample = frame(&back).to_vec();
    assert_eq!(sample.len(), 55);
    assert!(matches!(
        reader.read(&sample),
        Err(LogReadRefusal::MissingContext)
    ));
    for bytes in &frames {
        assert!(matches!(reader.read(bytes), Ok(LogFrame::Context { .. })));
    }
    let mut old = PreparedRecordingChunk::prepare(context().producer, &contract).unwrap();
    old.append(17, 100, RecordingDisposition::Available, Some(&canonical))
        .unwrap();
    assert_eq!(&sample[34..], old.samples[0]);
    commit(&mut back);
    assert_eq!(back.next_sequence(), 18);
    reader.note_media_gap();
    match reader.read(&sample).unwrap() {
        LogFrame::Sample {
            sample,
            media_gap_before,
        } => {
            assert!(media_gap_before);
            assert_eq!(sample.sequence, 17);
            assert_eq!(sample.sampled_at_boot_ms, 100);
            assert_eq!(sample.observation, Some(canonical.as_slice()));
        }
        _ => panic!(),
    }
    assert!(matches!(
        reader.read(&sample),
        Err(LogReadRefusal::Sequence)
    ));
    let mut later = sample.clone();
    later[34..42].copy_from_slice(&19u64.to_le_bytes());
    match reader.read(&later).unwrap() {
        LogFrame::Sample { sample, .. } => assert_eq!(sample.gap_before, Some((18, 18))),
        _ => panic!(),
    }
    let mut incomplete = PreparedLogReader::prepare_content(&contract).unwrap();
    assert!(matches!(
        incomplete.read(&frames[1]),
        Err(LogReadRefusal::Discontinuity)
    ));
    incomplete.read(&frames[0]).unwrap();
    assert_eq!(incomplete.finish(), Err(LogReadRefusal::MissingContext));

    let mut bad = frames[0].clone();
    bad[40] ^= 1;
    assert!(matches!(
        reader.read(&bad),
        Ok(LogFrame::Context { complete: false })
    ));
    for bytes in &frames[1..frames.len() - 1] {
        reader.read(bytes).unwrap();
    }
    assert!(matches!(
        reader.read(frames.last().unwrap()),
        Err(LogReadRefusal::ContextDigest)
    ));
    assert!(matches!(
        reader.read(&sample),
        Err(LogReadRefusal::MissingContext)
    ));
}
#[test]
fn refusal_cancellation_and_steady_encoding_allocate_nothing() {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let canonical = forms.unpack(&[0; 4]).unwrap().to_vec();
    let mut back = RecordingLogBack::prepare(context(), &contract, 1, 2, now).unwrap();
    let ((), allocations) = allocation_probe::observe(|| {
        assert!(matches!(
            stage(&mut back, &canonical, Some(200), 0).0,
            StepOutcome::Fail(Failure {
                code: FailureCode::WorkBudgetExhausted,
                ..
            })
        ));
        commit(&mut back);
        assert_eq!(back.ordinal, 0);
        assert!(matches!(
            stage(&mut back, b"invalid", Some(200), 16).0,
            StepOutcome::Fail(_)
        ));
        commit(&mut back);
        assert_eq!(back.ordinal, 0);
        while back.ordinal < back.context.parts() {
            assert_eq!(
                stage(&mut back, &canonical, Some(200), 16).0,
                StepOutcome::Progress
            );
            commit(&mut back);
        }
        assert!(matches!(
            stage(&mut back, &canonical, Some(1), 16).0,
            StepOutcome::Fail(Failure {
                code: FailureCode::StorageExhausted,
                ..
            })
        ));
        commit(&mut back);
        assert_eq!(back.next_sequence(), 1);
        assert_eq!(
            stage(&mut back, &canonical, Some(200), 16).0,
            StepOutcome::Progress
        );
        commit(&mut back);
        assert_eq!(back.next_sequence(), 2);
        assert_eq!(
            stage(&mut back, &canonical, Some(200), 16).0,
            StepOutcome::Progress
        );
        <RecordingLogBack as StepBack<1>>::cancel(&mut back);
        commit(&mut back);
        assert_eq!(back.next_sequence(), 2);
        assert!(matches!(
            stage(&mut back, &canonical, Some(200), 16).0,
            StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                ..
            })
        ));
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    assert_eq!(back.retained_backing_bytes(), 0);
}

#[test]
fn content_reader_recovers_unknown_boots_without_granting_them_authority() {
    let contract = ObservationContract::prepare().unwrap();
    let mut forms = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let canonical = forms.unpack(&[0; 4]).unwrap().to_vec();
    let mut reader = PreparedLogReader::prepare_content(&contract).unwrap();
    static SECOND_BOOT: LazyLock<BootId> = LazyLock::new(|| "boot/second".into());
    for boot in [context().producer.boot, &SECOND_BOOT] {
        let mut ctx = context();
        ctx.producer.boot = boot;
        let mut back = RecordingLogBack::prepare(ctx, &contract, 3, 1, now).unwrap();
        assert!(reader.context().is_none() || reader.context().unwrap().boot != boot.as_str());
        while back.ordinal < back.context.parts() {
            assert_eq!(
                stage(&mut back, &canonical, Some(200), 16).0,
                StepOutcome::Progress
            );
            assert!(matches!(
                reader.read(frame(&back)),
                Ok(LogFrame::Context { .. })
            ));
            commit(&mut back);
        }
        let recovered = reader.context().unwrap();
        assert_eq!(recovered.boot, boot.as_str());
        assert_eq!(recovered.body, "body/log");
        assert_eq!(recovered.first_sequence, 3);
        assert_eq!(
            stage(&mut back, &canonical, Some(200), 16).0,
            StepOutcome::Progress
        );
        assert!(matches!(
            reader.read(frame(&back)),
            Ok(LogFrame::Sample { .. })
        ));
        commit(&mut back);
        assert_eq!(
            stage(&mut back, &canonical, Some(200), 16).0,
            StepOutcome::Complete
        );
    }
}
