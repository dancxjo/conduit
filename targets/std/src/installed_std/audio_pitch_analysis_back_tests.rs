use super::*;
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_kernel::ValueRef;
fn operation() -> AudioPitchAnalysisBack {
    AudioPitchAnalysisBack {
        encoder: PreparedPcmPitchObservationEncoder::prepare(
            PcmPitchProfile::new(7, 8000, 1024, 0, 1024, 8, 256).unwrap(),
        )
        .unwrap(),
        remaining: 1,
        closed: false,
    }
}
fn pcm() -> Vec<u8> {
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        8000,
        PcmChannelLayout::Mono,
        1024,
        7,
        123,
        false,
    )
    .unwrap()
    .encode_frame(&[0; 4096])
    .unwrap()
}
fn step(
    operation: &mut AudioPitchAnalysisBack,
    bytes: &[u8],
    ready: bool,
) -> (StepOutcome, StepIo<1>) {
    let value = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    let mut io = StepIo::test_frame(
        [Some(value)],
        [false],
        [ready.then_some(operation.encoder.maximum_encoded_len() as u32)],
        None,
        8,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (outcome, io)
}
#[test]
fn pressure_preserves_input_output_and_budget_then_one_observation_consumes_once() {
    let mut operation = operation();
    let bytes = pcm();
    let before = operation.encoder.encoded().to_vec();
    let (outcome, io) = step(&mut operation, &bytes, false);
    assert_eq!(outcome, StepOutcome::Await);
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(operation.encoder.encoded(), before);
    assert_eq!(operation.remaining, 1);
    let (outcome, io) = step(&mut operation, &bytes, true);
    assert_eq!(outcome, StepOutcome::Progress);
    assert!(io.test_consumed(PortId(0)));
    assert_eq!(operation.remaining, 0);
    let (outcome, io) = step(&mut operation, &bytes, true);
    assert_eq!(outcome, fail(FailureCode::WorkBudgetExhausted, 1));
    assert!(!io.test_consumed(PortId(0)));
}
#[test]
fn cancellation_and_normal_close_do_not_fabricate_an_observation() {
    let mut operation = operation();
    let bytes = pcm();
    <AudioPitchAnalysisBack as StepBack<1>>::cancel(&mut operation);
    assert_eq!(step(&mut operation, &bytes, true).0, StepOutcome::Complete);
    let mut operation = self::operation();
    let mut io = StepIo::test_frame(
        [None],
        [true],
        [Some(operation.encoder.maximum_encoded_len() as u32)],
        None,
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
}
