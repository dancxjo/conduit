use super::*;
use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_kernel::ValueRef;
fn operation() -> AudioSpectralAnalysisBack {
    let geometry = PcmSpectralGeometry::new(64, 32, 16).unwrap();
    let profile =
        PcmSpectralProfile::new(geometry, 7, 8000, PcmSpectralWindow::PeriodicHann).unwrap();
    AudioSpectralAnalysisBack {
        encoder: PreparedPcmSpectrumObservationEncoder::prepare(profile).unwrap(),
        remaining: 1,
        window_index: 0,
        closed: false,
    }
}
fn pcm() -> Vec<u8> {
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        8000,
        PcmChannelLayout::Mono,
        64,
        7,
        123,
        false,
    )
    .unwrap()
    .encode_frame(&[0; 256])
    .unwrap()
}
fn step(
    operation: &mut AudioSpectralAnalysisBack,
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
        [ready.then_some(operation.encoder.encoded_len() as u32)],
        None,
        8,
    );
    let outcome = operation.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (outcome, io)
}
#[test]
fn pressure_keeps_original_input_and_budget_until_last_row() {
    let mut operation = operation();
    let bytes = pcm();
    for index in 0..3 {
        let before = operation.encoder.encoded().to_vec();
        let (outcome, io) = step(&mut operation, &bytes, false);
        assert_eq!(outcome, StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(operation.encoder.encoded(), before);
        assert_eq!(operation.window_index, index);
        assert_eq!(operation.remaining, 1);
        let (outcome, io) = step(&mut operation, &bytes, true);
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(io.test_consumed(PortId(0)), index == 2);
    }
    assert_eq!(operation.remaining, 0);
    assert_eq!(operation.window_index, 0);
    let (outcome, io) = step(&mut operation, &bytes, true);
    assert_eq!(outcome, fail(FailureCode::WorkBudgetExhausted, 1));
    assert!(!io.test_consumed(PortId(0)));
}
#[test]
fn cancellation_prevents_remaining_rows_and_close_has_no_extra_output() {
    let bytes = pcm();
    let mut operation = operation();
    assert_eq!(step(&mut operation, &bytes, true).0, StepOutcome::Progress);
    <AudioSpectralAnalysisBack as StepBack<1>>::cancel(&mut operation);
    let before = operation.encoder.encoded().to_vec();
    let (outcome, io) = step(&mut operation, &bytes, true);
    assert_eq!(outcome, StepOutcome::Complete);
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(operation.encoder.encoded(), before);
    let mut operation = self::operation();
    let mut io = StepIo::test_frame(
        [None],
        [true],
        [Some(operation.encoder.encoded_len() as u32)],
        None,
        8,
    );
    assert_eq!(
        operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
}
