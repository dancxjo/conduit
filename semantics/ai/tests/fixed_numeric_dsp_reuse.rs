#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp_back::FixedDspBack, fixed_numeric_dsp_catalog::*,
    fixed_numeric_integer_conversion::FixedIntegerConversionBack,
    operation_owners::fixed_numeric::FixedNumericOperationFactory,
};
use conduit_kernel::{
    scheduler::{
        prepared_session::ReusableStepBack, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    FailureCode, FixedValueStore, PortId, ValueRef,
};
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "common/fixed_dsp.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
fn frame(bytes: &[u8], ready: bool) -> StepIo<PORTS> {
    StepIo::test_frame(
        core::array::from_fn(|i| {
            (i == 0).then_some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: bytes.len() as u32,
            })
        }),
        [false; PORTS],
        core::array::from_fn(|i| (i == 0 && ready).then_some(16384)),
        None,
        2,
    )
}
fn inputs(bytes: &[u8]) -> StepInputBytes<'_, PORTS> {
    StepInputBytes::test_frame(core::array::from_fn(|i| (i == 0).then_some(bytes)), None)
}
trait Counted: ReusableStepBack<PORTS, Next = ()> {
    fn frames(&self) -> u64;
}
impl Counted for FixedDspBack<1, 1> {
    fn frames(&self) -> u64 {
        self.committed_frames()
    }
}
impl Counted for FixedIntegerConversionBack<1> {
    fn frames(&self) -> u64 {
        self.committed_frames()
    }
}
fn exercise(mut back: impl Counted, values: &[(Vec<u8>, f32)]) {
    let store = FixedValueStore::<1, 4>::new(4).unwrap();
    let decoder =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    let (_, observed) = allocation_probe::observe(|| {
        for i in 0..64 {
            let (bytes, expected) = &values[i % values.len()];
            assert_eq!(back.frames(), i as u64);
            assert!(!back.preflight_next(&(), &store));
            let mut blocked = frame(bytes, false);
            assert_eq!(back.step(&mut blocked, &inputs(bytes)), StepOutcome::Await);
            assert!(!blocked.test_consumed(PortId(0)));
            let mut ready = frame(bytes, true);
            assert_eq!(back.step(&mut ready, &inputs(bytes)), StepOutcome::Progress);
            assert!(
                !back.preflight_next(&(), &store),
                "unacknowledged staging is not reusable"
            );
            let mut actual = [0.];
            decoder
                .decode(back.prepared_output(PortId(0)).unwrap(), &mut actual)
                .unwrap();
            assert_eq!(actual, [*expected]);
            back.step_committed();
            assert_eq!(back.frames(), i as u64 + 1);
            assert!(back.preflight_next(&(), &store));
            back.begin_next(());
            assert_eq!(
                back.frames(),
                i as u64 + 1,
                "rearm preserves the finite lifetime counter"
            );
            assert!(!back.preflight_next(&(), &store));
            assert!(
                back.prepared_output(PortId(0)).is_none(),
                "old output stays private"
            );
        }
        back.cancel();
        assert!(!back.preflight_next(&(), &store));
        let mut io = frame(&values[0].0, true);
        assert!(matches!(
            back.step(&mut io, &inputs(&values[0].0)),
            StepOutcome::Fail(conduit_kernel::Failure {
                code: FailureCode::Cancelled,
                ..
            })
        ));
    });
    assert_eq!(
        (
            observed.allocations,
            observed.reallocations,
            observed.deallocations
        ),
        (0, 0, 0)
    );
}
#[test]
fn value_reuse_preserves_codec_custody_counter_and_ack_boundary_without_heap_activity() {
    let plan = fixture::plan(FixedDspOperation::Sqrt1, false);
    let back = FixedDspBack::<1, 1>::prepare_planned::<PORTS>(
        fixture::gear(&plan),
        2,
        FixedDspOperation::Sqrt1,
        false,
    )
    .unwrap();
    let mut codec =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    let values: Vec<_> = [(4., 2.), (9., 3.), (16., 4.)]
        .into_iter()
        .map(|(input, output)| (codec.encode(&[input]).unwrap().to_vec(), output))
        .collect();
    exercise(back, &values);
    let plan = fixture::conversion_plan(IntegerConversion::U16, false);
    let back = FixedIntegerConversionBack::<1>::prepare_planned::<PORTS>(
        fixture::gear(&plan),
        2,
        IntegerConversion::U16,
        false,
    )
    .unwrap();
    let values: Vec<_> = [0u16, 32, u16::MAX]
        .into_iter()
        .map(|v| (v.to_le_bytes().to_vec(), f32::from(v)))
        .collect();
    exercise(back, &values);
}
#[test]
fn factory_value_reuse_preserves_selection_and_refuses_flow_or_foreign_artifact() {
    let store = FixedValueStore::<1, 4>::new(4).unwrap();
    for flow in [false, true] {
        for plan in [
            fixture::plan(FixedDspOperation::Sqrt1, flow),
            fixture::conversion_plan(IntegerConversion::U16, flow),
        ] {
            let gear = fixture::gear(&plan);
            let owner =
                FixedNumericOperationFactory::for_plan(&plan, &std::collections::BTreeMap::new())
                    .unwrap()
                    .into_iter()
                    .find(|f| {
                        conduit_composite::KernelOperationFactory::implementation_id(f).as_str()
                            == DSP_IMPLEMENTATION
                    })
                    .unwrap();
            if flow {
                assert!(owner.prepare_reusable(gear).is_err());
            } else {
                let back = owner.prepare_reusable(gear).unwrap();
                assert!(!back.preflight_next(&(), &store));
                let mut foreign = gear.clone();
                foreign.artifact_id = "foreign/dsp".into();
                assert!(owner.prepare_reusable(&foreign).is_err());
            }
        }
    }
}
#[test]
fn normally_closed_flow_cannot_rearm_as_a_value_invocation() {
    let plan = fixture::plan(FixedDspOperation::Sqrt1, true);
    let mut back = FixedDspBack::<1, 1>::prepare_planned::<PORTS>(
        fixture::gear(&plan),
        2,
        FixedDspOperation::Sqrt1,
        true,
    )
    .unwrap();
    let store = FixedValueStore::<1, 4>::new(4).unwrap();
    let mut eof = StepIo::test_frame(
        [None; PORTS],
        core::array::from_fn(|i| i == 0),
        [None; PORTS],
        None,
        2,
    );
    assert_eq!(
        back.step(&mut eof, &StepInputBytes::test_frame([None; PORTS], None)),
        StepOutcome::Complete
    );
    assert!(!<FixedDspBack<1, 1> as ReusableStepBack<PORTS>>::preflight_next(&back, &(), &store));
    <FixedDspBack<1, 1> as StepBack<PORTS>>::cancel(&mut back);
    assert!(!<FixedDspBack<1, 1> as ReusableStepBack<PORTS>>::preflight_next(&back, &(), &store));
}
