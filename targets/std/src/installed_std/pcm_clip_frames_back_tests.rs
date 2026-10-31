use super::*;
use conduit_audio::{encode_pcm_clip, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
fn frame(start: u64, samples: &[i16]) -> Vec<u8> {
    PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        16000,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        7,
        start,
        false,
    )
    .unwrap()
    .encode_frame(
        &samples
            .iter()
            .flat_map(|n| n.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn value(bytes: &[u8]) -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    }
}
fn step(
    back: &mut PcmClipFramesBack,
    bytes: &[u8],
    value: ValueRef,
    ready: bool,
) -> (StepOutcome, StepIo<1>) {
    let mut io = StepIo::test_frame(
        [Some(value)],
        [false],
        [ready.then_some(PCM_CLIP_FRAMES_MAXIMUM_OUTPUT_BYTES)],
        None,
        8,
    );
    let result = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (result, io)
}
#[test]
fn full_drain_retains_original_input_under_pressure_and_preserves_short_tail_without_allocation() {
    let first = frame(31, &[1, 2, 3, 4]);
    let tail = frame(35, &[5]);
    let bytes = encode_pcm_clip(&[&first, &tail]).unwrap();
    let mut back = PcmClipFramesBack::new();
    let capacity = back.allocation_capacity();
    let allocations = crate::allocation_probe::begin();
    for (index, expected) in [&first, &tail].into_iter().enumerate() {
        for _ in 0..3 {
            let (outcome, io) = step(&mut back, &bytes, value(&bytes), false);
            assert_eq!(outcome, StepOutcome::Await);
            assert!(!io.test_consumed(PortId(0)));
            assert_eq!(back.next, index);
        }
        let (outcome, io) = step(&mut back, &bytes, value(&bytes), true);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(io.test_consumed(PortId(0)), index == 1);
        assert_eq!(
            <PcmClipFramesBack as StepBack<1>>::prepared_output(&back, PortId(0)).unwrap(),
            expected
        );
    }
    assert_eq!(
        step(&mut back, &bytes, value(&bytes), true).0,
        StepOutcome::Complete
    );
    assert_eq!(allocations.finish(), 0);
    assert_eq!(back.allocation_capacity(), capacity);
}
#[test]
fn malformed_tail_refuses_before_first_output_or_input_consumption() {
    let first = frame(31, &[1, 2]);
    let tail = frame(33, &[3]);
    let mut bytes = encode_pcm_clip(&[&first, &tail]).unwrap();
    bytes.pop();
    let mut back = PcmClipFramesBack::new();
    let allocations = crate::allocation_probe::begin();
    let (outcome, io) = step(&mut back, &bytes, value(&bytes), true);
    assert_eq!(outcome, invalid_clip(PcmClipError::MalformedFrame));
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!((back.output_len, back.next, back.source), (0, 0, None));
    assert_eq!(allocations.finish(), 0);
}
#[test]
fn cancellation_emits_no_remaining_frames_and_replaced_input_is_not_resumed() {
    let first = frame(31, &[1, 2]);
    let tail = frame(33, &[3]);
    let bytes = encode_pcm_clip(&[&first, &tail]).unwrap();
    let mut back = PcmClipFramesBack::new();
    assert_eq!(
        step(&mut back, &bytes, value(&bytes), true).0,
        StepOutcome::Progress
    );
    let foreign = ValueRef {
        generation: 2,
        ..value(&bytes)
    };
    assert_eq!(
        step(&mut back, &bytes, foreign, true).0,
        fail(FailureCode::InvalidLifecycle, 2)
    );
    assert_eq!(back.next, 1);
    <PcmClipFramesBack as StepBack<1>>::cancel(&mut back);
    let (outcome, io) = step(&mut back, &bytes, value(&bytes), true);
    assert_eq!(outcome, StepOutcome::Complete);
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(back.output_len, 0);
}
#[test]
fn maximum_stereo_float_block_fits_the_exact_semantic_output_bound() {
    let bytes = PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        192000,
        PcmChannelLayout::StereoLeftRight,
        2048,
        7,
        0,
        false,
    )
    .unwrap()
    .encode_frame(&[0; 2048 * 2 * 4])
    .unwrap();
    assert_eq!(bytes.len(), PCM_CLIP_FRAMES_MAXIMUM_OUTPUT_BYTES as usize);
    let clip = encode_pcm_clip(&[&bytes]).unwrap();
    let mut back = PcmClipFramesBack::new();
    assert_eq!(
        step(&mut back, &clip, value(&clip), true).0,
        StepOutcome::Progress
    );
    assert_eq!(&back.output[..back.output_len], bytes);
}
