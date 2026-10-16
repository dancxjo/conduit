use conduit_audio::{AudioPcm16PackingInput, PcmFrameHeader};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_std_host::flow_activation::PreparedPcm16PackingBack as Back;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn input() -> Vec<u8> {
    AudioPcm16PackingInput::block(
        1,
        false,
        8000,
        BoundedSequence::try_from_iter([24]).unwrap(),
        0,
    )
    .unwrap()
    .encode()
    .unwrap()
}
fn frame(input: &[u8], ready: bool) -> (StepIo<1>, StepInputBytes<'_, 1>) {
    (
        StepIo::test_frame(
            [Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: input.len() as u32,
            })],
            [false],
            [ready.then_some(2077)],
            None,
            8,
        ),
        StepInputBytes::test_frame([Some(input)], None),
    )
}
#[test]
fn pressure_retry_commit_budget_and_cancel_use_fixed_storage() {
    let input = input();
    let mut back = Back::prepare(1).unwrap();
    let (_, observed) = allocation_probe::observe(|| {
        let (mut io, bytes) = frame(&input, false);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        let (mut io, bytes) = frame(&input, true);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        let output = <Back as StepBack<1>>::prepared_output(&back, PortId(0)).unwrap();
        let (header, payload) = PcmFrameHeader::decode_frame(output).unwrap();
        assert_eq!(header.frame_count(), 1);
        assert_eq!(payload, 24_i16.to_le_bytes());
        // Rejected transactions must neither spend an input nor expose stale output.
        let (mut io, bytes) = frame(&input, false);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(<Back as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
        let (mut io, bytes) = frame(&input, true);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        <Back as StepBack<1>>::step_committed(&mut back);
        let (mut io, bytes) = frame(&input, true);
        assert!(matches!(back.step(&mut io, &bytes), StepOutcome::Fail(_)));
        assert!(!io.test_consumed(PortId(0)));
        <Back as StepBack<1>>::cancel(&mut back);
        assert!(<Back as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
        let (mut io, bytes) = frame(&input, true);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Complete);
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    assert!(Back::prepare(0).is_err());
    assert!(Back::prepare(65).is_err());
}
