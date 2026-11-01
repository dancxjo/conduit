#![cfg(feature = "kernel-operation-owners")]
use conduit_ai::{
    fixed_numeric_catalog::*, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp_catalog::*,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
#[path = "common/fixed_dsp.rs"]
mod fixture;
use fixture::{conversion_plan, frame, gear, plan};
#[test]
fn selected_integer_converters_preserve_signed_scale_and_stage_only_on_commit() {
    use conduit_ai::{
        fixed_numeric_i16_codec::FixedI16VectorCodec,
        fixed_numeric_integer_conversion::FixedIntegerConversionBack,
    };
    let plan = conversion_plan(IntegerConversion::U16, true);
    let mut back = FixedIntegerConversionBack::<1>::prepare_planned::<4>(
        gear(&plan),
        3,
        IntegerConversion::U16,
        true,
    )
    .unwrap();
    let decoder =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    for value in [0u16, 32, 255, u16::MAX] {
        let bytes = value.to_le_bytes();
        let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
        let mut blocked = frame(&bytes, false);
        assert_eq!(back.step(&mut blocked, &inputs), StepOutcome::Await);
        assert!(!blocked.test_consumed(PortId(0)));
        let before = back.committed_frames();
        let mut io = frame(&bytes, true);
        assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(back.committed_frames(), before);
        let mut out = [0.];
        decoder
            .decode(
                <FixedIntegerConversionBack<1> as StepBack<4>>::prepared_output(&back, PortId(0))
                    .unwrap(),
                &mut out,
            )
            .unwrap();
        assert_eq!(out, [f32::from(value)]);
        <FixedIntegerConversionBack<1> as StepBack<4>>::step_committed(&mut back);
        assert_eq!(back.committed_frames(), before + 1);
    }
    let plan = conversion_plan(IntegerConversion::I16Vector160, false);
    let mut back = FixedIntegerConversionBack::<160>::prepare_planned::<4>(
        gear(&plan),
        3,
        IntegerConversion::I16Vector160,
        false,
    )
    .unwrap();
    let mut codec =
        FixedI16VectorCodec::<160>::prepare(&fixed_numeric_type("NumericI16Vector160").unwrap())
            .unwrap();
    let signed = core::array::from_fn(|i| [i16::MIN, -1, 0, i16::MAX][i % 4]);
    let bytes = codec.encode(&signed);
    let inputs = StepInputBytes::test_frame([Some(bytes), None, None, None], None);
    let mut io = frame(bytes, true);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    let mut out = [0.; 160];
    FixedF32VectorCodec::<160>::prepare(&fixed_numeric_type("NumericF32Vector160").unwrap())
        .unwrap()
        .decode(
            <FixedIntegerConversionBack<160> as StepBack<4>>::prepared_output(&back, PortId(0))
                .unwrap(),
            &mut out,
        )
        .unwrap();
    assert_eq!(out, signed.map(f32::from));
    <FixedIntegerConversionBack<160> as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Complete);
}

#[test]
fn finite_admission_checks_exact_raw_envelope_and_nonfinite_before_consumption() {
    use conduit_ai::fixed_numeric_integer_conversion::FixedIntegerConversionBack;
    let plan = conversion_plan(IntegerConversion::FiniteVector(1), false);
    let mut back = FixedIntegerConversionBack::<1>::prepare_planned::<4>(
        gear(&plan),
        3,
        IntegerConversion::FiniteVector(1),
        false,
    )
    .unwrap();
    let raw_type = fixed_numeric_type("NumericRawF32Vector1").unwrap();
    assert!(FixedF32VectorCodec::<1>::prepare(&raw_type).is_err());
    let mut raw = FixedF32VectorCodec::<1>::prepare_raw(&raw_type).unwrap();
    let bytes = raw.encode(&[f32::from_bits(0x3f010203)]).unwrap().to_vec();
    let mut invalid = bytes.clone();
    let offset = invalid.windows(4).position(|w| w == [3, 2, 1, 63]).unwrap();
    invalid[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    let mut io = frame(&invalid, true);
    let inputs = StepInputBytes::test_frame([Some(&invalid), None, None, None], None);
    assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Fail(_)));
    assert!(!io.test_consumed(PortId(0)));
    assert_eq!(back.committed_frames(), 0);
    assert!(
        <FixedIntegerConversionBack<1> as StepBack<4>>::prepared_output(&back, PortId(0)).is_none()
    );
    let mut io = frame(&bytes, true);
    let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    let finite =
        FixedF32VectorCodec::<1>::prepare(&fixed_numeric_type("NumericF32Vector1").unwrap())
            .unwrap();
    let mut out = [0.];
    finite
        .decode(
            <FixedIntegerConversionBack<1> as StepBack<4>>::prepared_output(&back, PortId(0))
                .unwrap(),
            &mut out,
        )
        .unwrap();
    assert_eq!(out, [f32::from_bits(0x3f010203)]);
}

#[test]
fn hosted_factory_selects_explicit_flow_dsp_and_finite_admission_offers() {
    use conduit_composite::KernelOperationFactory;
    for plan in [
        plan(FixedDspOperation::Sqrt1, true),
        conversion_plan(IntegerConversion::FiniteVector(20), true),
        conversion_plan(IntegerConversion::FiniteVector(256), false),
        conversion_plan(IntegerConversion::I16Vector80, true),
    ] {
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
        let _back = factory.prepare(gear(&plan), &mut store).unwrap();
    }
}

#[test]
fn finite256_refuses_malformed_and_nonfinite_without_partial_output() {
    use conduit_ai::fixed_numeric_integer_conversion::FixedIntegerConversionBack;
    let operation = IntegerConversion::FiniteVector(256);
    let plan = conversion_plan(operation, false);
    let mut back =
        FixedIntegerConversionBack::<256>::prepare_planned::<4>(gear(&plan), 3, operation, false)
            .unwrap();
    let ty = fixed_numeric_type("NumericRawF32Vector256").unwrap();
    let mut raw = FixedF32VectorCodec::<256>::prepare_raw(&ty).unwrap();
    let values = core::array::from_fn(|i| {
        if i == 255 {
            f32::from_bits(0x3f010203)
        } else {
            i as f32
        }
    });
    let bytes = raw.encode(&values).unwrap().to_vec();
    let mut nonfinite = bytes.clone();
    let offset = nonfinite
        .windows(4)
        .rposition(|w| w == [3, 2, 1, 63])
        .unwrap();
    nonfinite[offset..offset + 4].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let mut malformed = bytes.clone();
    malformed[0] ^= 255;
    for invalid in [&nonfinite[..], &malformed[..], &bytes[..bytes.len() - 1]] {
        let mut decoded = [9.; 256];
        assert!(raw.decode(invalid, &mut decoded).is_err());
        assert_eq!(decoded, [9.; 256]);
        let mut io = frame(invalid, true);
        let inputs = StepInputBytes::test_frame([Some(invalid), None, None, None], None);
        assert!(matches!(back.step(&mut io, &inputs), StepOutcome::Fail(_)));
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(back.committed_frames(), 0);
        assert!(
            <FixedIntegerConversionBack<256> as StepBack<4>>::prepared_output(&back, PortId(0))
                .is_none()
        );
    }
    let mut io = frame(&bytes, false);
    let inputs = StepInputBytes::test_frame([Some(&bytes), None, None, None], None);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Await);
    assert!(!io.test_consumed(PortId(0)));
    let mut io = frame(&bytes, true);
    assert_eq!(back.step(&mut io, &inputs), StepOutcome::Progress);
    let mut output = [0.; 256];
    FixedF32VectorCodec::<256>::prepare(&fixed_numeric_type("NumericF32Vector256").unwrap())
        .unwrap()
        .decode(
            <FixedIntegerConversionBack<256> as StepBack<4>>::prepared_output(&back, PortId(0))
                .unwrap(),
            &mut output,
        )
        .unwrap();
    assert_eq!(output, values);
    assert_eq!(back.committed_frames(), 0);
    <FixedIntegerConversionBack<256> as StepBack<4>>::step_committed(&mut back);
    assert_eq!(back.committed_frames(), 1);
}

#[path = "fixed_numeric_dsp_conversion/flow_closure.rs"]
mod flow_closure;
