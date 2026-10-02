use super::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostCallOutcome,
};

fn value(slot: u16, byte_len: u32) -> ValueRef {
    ValueRef {
        slot,
        generation: 1,
        byte_len,
    }
}

fn input_step(operation: &mut SpeechSynthesisBack, value: ValueRef) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [Some(value)],
        [false],
        [Some(conduit_std_offers::SPEECH_PCM_BLOCK_BYTES)],
        None,
        8,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

fn completion_step(
    operation: &mut SpeechSynthesisBack,
    request: u32,
    output: Option<BoundedValueRef>,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [Some(conduit_std_offers::SPEECH_PCM_BLOCK_BYTES)],
        Some((
            RequestId(request),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output,
                failure: None,
            },
        )),
        8,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([None], None));
    (outcome, io)
}

#[test]
fn operation_pulls_one_block_only_after_each_emit_commits() {
    let mut operation = SpeechSynthesisBack {
        continuation: value(9, 1),
        pending: None,
        next_request: 0,
        emitted_blocks: 0,
        maximum_blocks: 2,
        streaming: false,
        input_closed: false,
        started: false,
        finished: false,
    };
    let (outcome, io) = input_step(&mut operation, value(1, 7));
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(0))
    );
    let output = BoundedValueRef::new(
        value(2, conduit_std_offers::SPEECH_PCM_BLOCK_BYTES),
        conduit_std_offers::SPEECH_PCM_BLOCK_BYTES,
    )
    .unwrap();
    let (outcome, io) = completion_step(&mut operation, 0, Some(output));
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(io.test_output(PortId(0)), Some(output.value));
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
}

#[test]
fn operation_refuses_a_provider_block_beyond_the_admitted_count() {
    let mut operation = SpeechSynthesisBack {
        continuation: value(9, 1),
        pending: Some(RequestId(2)),
        next_request: 3,
        emitted_blocks: 2,
        maximum_blocks: 2,
        streaming: false,
        input_closed: false,
        started: true,
        finished: false,
    };
    let output =
        BoundedValueRef::new(value(3, 1), conduit_std_offers::SPEECH_PCM_BLOCK_BYTES).unwrap();
    assert!(matches!(
        completion_step(&mut operation, 2, Some(output)).0,
        StepOutcome::Fail(Failure {
            code: FailureCode::WorkBudgetExhausted,
            ..
        })
    ));
}

#[test]
fn streaming_operation_synthesizes_ordered_segments_until_input_closes() {
    let mut operation = SpeechSynthesisBack {
        continuation: value(9, 1),
        pending: None,
        next_request: 0,
        emitted_blocks: 0,
        maximum_blocks: 4,
        streaming: true,
        input_closed: false,
        started: false,
        finished: false,
    };
    let (outcome, io) = input_step(&mut operation, value(1, 128));
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(0))
    );
    assert_eq!(
        completion_step(&mut operation, 0, None).0,
        StepOutcome::Progress
    );
    let (outcome, io) = input_step(&mut operation, value(2, 128));
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(
        io.test_host_request().map(|request| request.0),
        Some(RequestId(1))
    );
    assert_eq!(
        completion_step(&mut operation, 1, None).0,
        StepOutcome::Progress
    );
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
    assert!(io.test_discards().contains(&Some(operation.continuation)));
}

#[test]
fn fake_provider_emits_three_canonical_contiguous_profile_blocks() {
    let blocks: [Vec<u8>; 3] = core::array::from_fn(|index| {
        let header = conduit_audio::PcmFrameHeader::new(
            conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
            22_050,
            conduit_audio::PcmChannelLayout::Mono,
            4,
            0x5350_4545_4348,
            index as u64 * 4,
            false,
        )
        .unwrap();
        header.encode_frame(&[0; 8]).unwrap()
    });
    let mut host = FakeSpeechHost {
        blocks,
        next: 0,
        active: false,
    };
    for (index, input) in [b"Rosehip".as_slice(), &[0], &[0]].into_iter().enumerate() {
        let block = host.execute(input).unwrap().unwrap();
        let (header, payload) = conduit_audio::PcmFrameHeader::decode_frame(block).unwrap();
        assert_eq!(header.sample_rate_hz, 22_050);
        assert_eq!(header.layout, conduit_audio::PcmChannelLayout::Mono);
        assert_eq!(header.start_frame, index as u64 * 4);
        assert_eq!(payload.len(), 8);
    }
    assert_eq!(host.execute(&[0]).unwrap(), None);
}
