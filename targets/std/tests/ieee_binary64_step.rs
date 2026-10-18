//! Shared-kernel Step protocol conformance; installed Plan proof is separate.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../src/numerical_host/ieee_encoding.rs"]
mod ieee_encoding;
#[allow(dead_code)] // MXCSR preservation probe is exercised by ieee_binary64_native.
#[path = "../src/numerical_host/ieee_native.rs"]
mod ieee_native;
#[path = "../src/numerical_host/ieee_step.rs"]
mod ieee_step;
use conduit_core::*;
use conduit_kernel::{scheduler::*, Failure, FailureCode, PortId, ValueRef};
use conduit_semantic_catalog::IeeeBinary64Configuration as Configuration;
use ieee_step::IeeeBinary64Back;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn output(back: &IeeeBinary64Back, port: PortId) -> Option<&[u8]> {
    <IeeeBinary64Back as StepBack<2>>::prepared_output(back, port)
}
fn io(output: Option<u32>, fuel: u16) -> StepIo<2> {
    StepIo::test_frame(
        [
            Some(ValueRef {
                slot: 0,
                generation: 0,
                byte_len: 8,
            }),
            Some(ValueRef {
                slot: 1,
                generation: 0,
                byte_len: 8,
            }),
        ],
        [false; 2],
        [output, None],
        None,
        fuel,
    )
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
fn inspect(bytes: &[u8], value: u64, flags: IeeeBinary64ExceptionFlags) {
    let result = validate_canonical_structured_value(bytes).unwrap();
    assert_eq!(
        result
            .record_field("value")
            .unwrap()
            .unwrap()
            .primitive_bytes(F64_INFO_ID)
            .unwrap(),
        value.to_le_bytes()
    );
    let f = result.record_field("flags").unwrap().unwrap();
    for (name, expected) in [
        ("invalid", flags.invalid),
        ("divide-by-zero", flags.divide_by_zero),
        ("overflow", flags.overflow),
        ("underflow", flags.underflow),
        ("inexact", flags.inexact),
    ] {
        assert_eq!(
            f.record_field(name)
                .unwrap()
                .unwrap()
                .primitive_bytes(BOOL_INFO_ID)
                .unwrap(),
            InfoBool::new(expected).encode()
        );
    }
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[test]
fn arithmetic_modes_stage_typed_results_atomically_without_warm_allocation() {
    for operation in [
        IeeeBinary64Operation::Add,
        IeeeBinary64Operation::Subtract,
        IeeeBinary64Operation::Multiply,
        IeeeBinary64Operation::Divide,
    ] {
        for rounding in [
            Binary64Rounding::NearestTiesEven,
            Binary64Rounding::TowardZero,
            Binary64Rounding::TowardPositive,
            Binary64Rounding::TowardNegative,
        ] {
            let left = 0x3ff0_0000_0000_0000u64;
            let right = 0x4008_0000_0000_0000u64;
            let expected = evaluate_ieee_binary64(
                operation,
                IeeeF64::from_bits(left),
                IeeeF64::from_bits(right),
                rounding,
                &mut ExactNumericWork::new(100_000_000),
            )
            .unwrap();
            let mut back = IeeeBinary64Back::new(Configuration::Arithmetic {
                operation,
                rounding,
            })
            .unwrap();
            let capacity = back.retained_capacity_bytes();
            let (left, right) = (left.to_le_bytes(), right.to_le_bytes());
            let bytes =
                StepInputBytes::test_frame([Some(left.as_slice()), Some(right.as_slice())], None);
            let mut frame = io(Some(back.output_bytes() as u32), 4);
            let ((), count) = allocation_probe::observe(|| {
                assert_eq!(back.step(&mut frame, &bytes), StepOutcome::Progress);
                assert!(frame.test_consumed(PortId(0)) && frame.test_consumed(PortId(1)));
                assert_eq!(frame.test_fuel_consumed(), 4);
                inspect(
                    output(&back, PortId(0)).unwrap(),
                    expected.value.bits(),
                    expected.flags,
                );
                assert!(output(&back, PortId(1)).is_none());
                <IeeeBinary64Back as StepBack<2>>::step_committed(&mut back);
                assert!(output(&back, PortId(0)).is_none());
                assert_eq!(back.step(&mut frame, &bytes), StepOutcome::Complete);
            });
            assert_eq!((count.allocations, count.reallocations), (0, 0));
            assert_eq!(capacity, back.retained_capacity_bytes());
        }
    }
}
#[test]
fn pressure_cancellation_malformed_input_and_fuel_remain_distinct() {
    let config = Configuration::Quiet {
        predicate: IeeeBinary64QuietPredicate::Equal,
    };
    let left = 0u64.to_le_bytes();
    let right = (1u64 << 63).to_le_bytes();
    let bytes = StepInputBytes::test_frame([Some(left.as_slice()), Some(right.as_slice())], None);
    let mut back = IeeeBinary64Back::new(config).unwrap();
    let mut frame = io(None, 4);
    assert_eq!(back.step(&mut frame, &bytes), StepOutcome::Await);
    assert_eq!(frame.test_fuel_consumed(), 0);
    assert!(!frame.test_consumed(PortId(0)) && !frame.test_consumed(PortId(1)));
    <IeeeBinary64Back as StepBack<2>>::cancel(&mut back);
    assert_eq!(back.step(&mut frame, &bytes), StepOutcome::Complete);
    assert!(output(&back, PortId(0)).is_none());
    let mut back = IeeeBinary64Back::new(config).unwrap();
    let mut frame = io(Some(8192), 0);
    assert_eq!(
        back.step(&mut frame, &bytes),
        StepOutcome::Fail(Failure {
            code: FailureCode::WorkBudgetExhausted,
            detail: 1
        })
    );
    assert!(!frame.test_consumed(PortId(0)) && !frame.test_consumed(PortId(1)));
    let malformed = StepInputBytes::test_frame([Some(&[0u8; 7][..]), Some(right.as_slice())], None);
    let mut back = IeeeBinary64Back::new(config).unwrap();
    let mut frame = io(Some(8192), 4);
    assert_eq!(
        back.step(&mut frame, &malformed),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 4
        })
    );
    assert!(!frame.test_consumed(PortId(0)) && !frame.test_consumed(PortId(1)));
    let mut back = IeeeBinary64Back::new(config).unwrap();
    let mut frame = io(Some(1), 4);
    assert_eq!(
        back.step(&mut frame, &bytes),
        StepOutcome::Fail(Failure {
            code: FailureCode::StorageExhausted,
            detail: 1
        })
    );
    assert!(output(&back, PortId(0)).is_none());
}
#[test]
fn quiet_numeric_comparison_treats_signed_zeros_and_signaling_nan_explicitly() {
    for (left, right, expected, invalid) in [
        (0u64, 1u64 << 63, true, false),
        (0x7ff0_0000_0000_0001, 0, false, true),
    ] {
        let mut back = IeeeBinary64Back::new(Configuration::Quiet {
            predicate: IeeeBinary64QuietPredicate::Equal,
        })
        .unwrap();
        let (left, right) = (left.to_le_bytes(), right.to_le_bytes());
        let bytes =
            StepInputBytes::test_frame([Some(left.as_slice()), Some(right.as_slice())], None);
        let mut frame = io(Some(8192), 4);
        assert_eq!(back.step(&mut frame, &bytes), StepOutcome::Progress);
        let result =
            validate_canonical_structured_value(output(&back, PortId(0)).unwrap()).unwrap();
        assert_eq!(
            result
                .record_field("value")
                .unwrap()
                .unwrap()
                .primitive_bytes(BOOL_INFO_ID)
                .unwrap(),
            InfoBool::new(expected).encode()
        );
        assert_eq!(
            result
                .record_field("flags")
                .unwrap()
                .unwrap()
                .record_field("invalid")
                .unwrap()
                .unwrap()
                .primitive_bytes(BOOL_INFO_ID)
                .unwrap(),
            InfoBool::new(invalid).encode()
        );
    }
}
