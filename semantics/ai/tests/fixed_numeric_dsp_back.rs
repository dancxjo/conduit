#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp_catalog::*,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
#[path = "common/fixed_dsp.rs"]
mod fixture;
use conduit_ai::fixed_numeric_dsp_back::FixedDspBack;
use fixture::{frame, gear, plan};
#[test]
fn selected_closing_flow_sqrt_stages_three_frames_and_refuses_domain_without_progress() {
    let plan = plan(FixedDspOperation::Sqrt1, true);
    let mut back =
        FixedDspBack::<1, 1>::prepare_planned::<4>(gear(&plan), 3, FixedDspOperation::Sqrt1, true)
            .unwrap();
    let ty = fixed_numeric_type("NumericF32Vector1").unwrap();
    let mut codec = FixedF32VectorCodec::<1>::prepare(&ty).unwrap();
    let decoder = FixedF32VectorCodec::<1>::prepare(&ty).unwrap();
    for (i, value) in [4., 9., 16.].into_iter().enumerate() {
        let bytes = codec.encode(&[value]).unwrap();
        let inputs = StepInputBytes::test_frame([Some(bytes), None, None, None], None);
        let mut blocked = frame(bytes, false);
        assert_eq!(back.step(&mut blocked, &inputs), StepOutcome::Await);
        assert!(!blocked.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), i as u64);
        let mut io = frame(bytes, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), i as u64);
        let mut output = [0.];
        decoder
            .decode(
                <FixedDspBack<1, 1> as StepBack<4>>::prepared_output(&back, PortId(0)).unwrap(),
                &mut output,
            )
            .unwrap();
        assert_eq!(output, [i as f32 + 2.]);
        <FixedDspBack<1, 1> as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_frames(), i as u64 + 1);
    }
    let bytes = codec.encode(&[-1.]).unwrap();
    let mut io = frame(bytes, true);
    assert!(matches!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(bytes), None, None, None], None)
        ),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidInput,
            detail: 2813
        })
    ));
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(back.committed_frames(), 3);
    <FixedDspBack<1, 1> as StepBack<4>>::cancel(&mut back);
    let mut io = frame(bytes, true);
    assert!(matches!(
        back.step(
            &mut io,
            &StepInputBytes::test_frame([Some(bytes), None, None, None], None)
        ),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::Cancelled,
            ..
        })
    ));
    assert_eq!(back.committed_frames(), 3);
    let mut foreign = gear(&plan).clone();
    foreign.implementation_id = ImplementationId::from("foreign/dsp");
    assert!(FixedDspBack::<1, 1>::prepare_planned::<4>(
        &foreign,
        3,
        FixedDspOperation::Sqrt1,
        true
    )
    .is_err());
    assert!(FixedDspBack::<1, 1>::prepare_planned::<4>(
        gear(&plan),
        3,
        FixedDspOperation::Sqrt1,
        false
    )
    .is_err());
}
#[test]
fn ordinary_ai_factory_prepares_exact_dft_and_dot_value_operations() {
    use conduit_composite::KernelOperationFactory;
    for operation in [FixedDspOperation::RealDft320, FixedDspOperation::Dot160] {
        let plan = plan(operation, false);
        let factory =
            conduit_ai::operation_owners::fixed_numeric::FixedNumericOperationFactory::for_plan(
                &plan,
                &std::collections::BTreeMap::new(),
            )
            .unwrap()
            .into_iter()
            .find(|factory| factory.implementation_id().as_str() == DSP_IMPLEMENTATION)
            .unwrap();
        let mut store = conduit_kernel::HostedValueStore::new(4, 16384, 65536).unwrap();
        let mut back = factory.prepare(gear(&plan), &mut store).unwrap();
        let mut input = if operation == FixedDspOperation::RealDft320 {
            let mut codec = FixedF32VectorCodec::<320>::prepare(
                &fixed_numeric_type("NumericF32Vector320").unwrap(),
            )
            .unwrap();
            let mut impulse = [0.; 320];
            impulse[0] = 1.;
            codec.encode(&impulse).unwrap().to_vec()
        } else {
            let mut codec = FixedF32VectorCodec::<160>::prepare(
                &fixed_numeric_type("NumericF32Vector160").unwrap(),
            )
            .unwrap();
            codec.encode(&[0.5; 160]).unwrap().to_vec()
        };
        let right = if operation == FixedDspOperation::Dot160 {
            let mut codec = FixedF32VectorCodec::<160>::prepare(
                &fixed_numeric_type("NumericF32Vector160").unwrap(),
            )
            .unwrap();
            Some(codec.encode(&[0.25; 160]).unwrap().to_vec())
        } else {
            None
        };
        let mut refs = [None; 16];
        refs[0] = Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: input.len() as u32,
        });
        if let Some(right) = &right {
            refs[1] = Some(ValueRef {
                slot: 1,
                generation: 1,
                byte_len: right.len() as u32,
            });
        }
        let mut io = StepIo::test_frame(
            refs,
            [false; 16],
            core::array::from_fn(|i| if i == 0 { Some(16384) } else { None }),
            None,
            16,
        );
        assert_eq!(
            back.step(
                &mut io,
                &StepInputBytes::test_frame(
                    core::array::from_fn(|i| match i {
                        0 => Some(input.as_slice()),
                        1 => right.as_deref(),
                        _ => None,
                    }),
                    None
                )
            ),
            StepOutcome::Progress
        );
        let bytes = back.prepared_output(PortId(0)).unwrap();
        if operation == FixedDspOperation::RealDft320 {
            let codec = FixedF32VectorCodec::<322>::prepare(
                &fixed_numeric_type("NumericF32Vector322").unwrap(),
            )
            .unwrap();
            let mut output = [0.; 322];
            codec.decode(bytes, &mut output).unwrap();
            for bin in output.as_chunks::<2>().0 {
                assert_eq!(bin, &[1., 0.]);
            }
        } else {
            let codec = FixedF32VectorCodec::<1>::prepare(
                &fixed_numeric_type("NumericF32Vector1").unwrap(),
            )
            .unwrap();
            let mut output = [0.];
            codec.decode(bytes, &mut output).unwrap();
            assert_eq!(output, [20.]);
        }
        back.step_committed();
        input.clear();
        let mut io = StepIo::test_frame(
            [None; 16],
            [false; 16],
            core::array::from_fn(|i| if i == 0 { Some(16384) } else { None }),
            None,
            16,
        );
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 16], None)),
            StepOutcome::Complete
        );
    }
}

#[path = "fixed_numeric_dsp_back/flow_closure.rs"]
mod flow_closure;
