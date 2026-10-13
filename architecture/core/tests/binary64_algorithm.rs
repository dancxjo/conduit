use conduit_core::projection::*;
use conduit_core::{
    Binary64AlgorithmDomain as Domain, Binary64AlgorithmLimits as Limits,
    Binary64AlgorithmPolicy as Policy, Binary64AlgorithmRefusal as Refusal,
    Binary64ErrorClaim as Claim, Binary64Operation as Op, Binary64Refusal,
    Binary64Rounding as Mode, ExactNumericRefusal, ExactNumericWork as Work, ExactRational as Q,
    IeeeF64, PreparedBinary64Algorithm as Prepared, PreparedBinary64Projection as Point,
};
fn work() -> Work {
    Work::new(100_000_000)
}
fn q(s: &str) -> Q {
    Q::parse_decimal(s, &mut work()).unwrap()
}
fn limits() -> Limits {
    Limits {
        maximum_steps: 32,
        maximum_retained_bytes: 1_000_000,
    }
}
fn prepare(program: &[Op], mode: Mode, error: Q) -> Result<Prepared, Refusal> {
    Prepared::prepare(program, mode, error, limits(), &mut work())
}
#[test]
fn intermediate_rounding_is_retained_and_final_exact_rounding_is_a_different_algorithm() {
    let program = [
        Op::Constant(Box::new(q("0.1"))),
        Op::Constant(Box::new(q("0.2"))),
        Op::Add { left: 0, right: 1 },
    ];
    let p = prepare(&program, Mode::NearestTiesEven, Q::ONE).unwrap();
    assert_eq!(p.source(), q("0.3"));
    assert_eq!(p.target().bits(), (0.1f64 + 0.2f64).to_bits());
    assert_eq!(
        p.error(),
        Claim::ExactDistance(Q::ratio(1, 22_517_998_136_852_480, &mut work()).unwrap())
    );
    let final_only = Point::prepare(q("0.3"), Mode::NearestTiesEven, Q::ONE, &mut work()).unwrap();
    assert_ne!(p.target(), final_only.target());
    assert!(p.steps().iter().all(|s| s.projection().status().inexact));
    assert_ne!(
        p.steps()[2].projection().source(),
        p.steps()[2].semantic_value()
    );
    assert!(p.spent_limb_work() > 0 && p.spent_limb_work() <= p.admitted_limb_work());
    assert!(p.retained_payload_bytes() > core::mem::size_of::<Prepared>());
}
#[test]
fn cancellation_and_reordering_have_distinct_certificates_and_tolerance_admission() {
    let a = q("10000000000000000");
    let original = [
        Op::Constant(Box::new(a)),
        Op::Constant(Box::new(Q::ONE)),
        Op::Constant(Box::new(a)),
        Op::Add { left: 0, right: 1 },
        Op::Subtract { left: 3, right: 2 },
    ];
    assert_eq!(
        prepare(&original, Mode::NearestTiesEven, Q::ZERO),
        Err(Refusal::Projection(Binary64Refusal::ToleranceExceeded))
    );
    let cancelled = prepare(&original, Mode::NearestTiesEven, Q::ONE).unwrap();
    assert_eq!(cancelled.source(), Q::ONE);
    assert_eq!(cancelled.target().bits(), 0);
    assert_eq!(cancelled.error(), Claim::ExactDistance(Q::ONE));
    assert!(cancelled.steps()[3].projection().status().inexact);
    assert!(!cancelled.steps()[4].projection().status().inexact);
    let reordered = [
        Op::Constant(Box::new(a)),
        Op::Constant(Box::new(Q::ONE)),
        Op::Constant(Box::new(a)),
        Op::Subtract { left: 0, right: 2 },
        Op::Add { left: 3, right: 1 },
    ];
    let exact = prepare(&reordered, Mode::NearestTiesEven, Q::ZERO).unwrap();
    assert_eq!(exact.source(), cancelled.source());
    assert_eq!(exact.target().bits(), 1.0f64.to_bits());
    assert_eq!(exact.error(), Claim::ExactDistance(Q::ZERO));
    assert_ne!(exact.steps(), cancelled.steps());
}
#[test]
fn directed_modes_and_signed_zero_follow_the_selected_operation_order() {
    let one = Op::IeeeConstant(IeeeF64::from_value(1.0));
    let neg_one = Op::IeeeConstant(IeeeF64::from_value(-1.0));
    for (mode, zero) in [
        (Mode::NearestTiesEven, 0),
        (Mode::TowardZero, 0),
        (Mode::TowardPositive, 0),
        (Mode::TowardNegative, 1 << 63),
    ] {
        let p = prepare(
            &[one.clone(), neg_one.clone(), Op::Add { left: 0, right: 1 }],
            mode,
            Q::ZERO,
        )
        .unwrap();
        assert_eq!(p.target().bits(), zero);
    }
    let nz = Op::IeeeConstant(IeeeF64::from_bits(1 << 63));
    for (operation, bits) in [
        (Op::Add { left: 0, right: 0 }, 1 << 63),
        (Op::Subtract { left: 0, right: 0 }, 0),
        (Op::Multiply { left: 0, right: 1 }, 1 << 63),
        (Op::Divide { left: 0, right: 1 }, 1 << 63),
        (Op::Negate { input: 0 }, 0),
    ] {
        let p = prepare(
            &[nz.clone(), one.clone(), operation],
            Mode::NearestTiesEven,
            Q::ZERO,
        )
        .unwrap();
        assert_eq!(p.target().bits(), bits);
    }
    let exact_third = Q::ratio(1, 3, &mut work()).unwrap();
    let program = [
        Op::Constant(Box::new(Q::ONE)),
        Op::Constant(Box::new(Q::from_i128(3))),
        Op::Divide { left: 0, right: 1 },
    ];
    let p = prepare(&program, Mode::NearestTiesEven, Q::ONE).unwrap();
    assert_eq!(p.source(), exact_third);
    assert_eq!(p.target().bits(), (1.0f64 / 3.0).to_bits());
}
#[test]
fn generic_projection_validates_end_to_end_loss_and_rejects_a_false_exact_claim() {
    let p = prepare(
        &[Op::Constant(Box::new(q("0.1")))],
        Mode::NearestTiesEven,
        Q::ONE,
    )
    .unwrap();
    let facts = p.projection_facts();
    assert!(Domain.validate(
        &p,
        Some(&p.target()),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    assert!(Policy {
        maximum_error: Q::ONE,
        rounding: Mode::NearestTiesEven
    }
    .permits(&p, &p.target(), &facts));
    assert!(!Policy {
        maximum_error: Q::ZERO,
        rounding: Mode::NearestTiesEven
    }
    .permits(&p, &p.target(), &facts));
    assert!(!Domain.validate(
        &p,
        Some(&IeeeF64::from_bits(0)),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    let false_exact = [
        ProjectionFact::Preserved {
            obligation: ProjectionText::new("mathematical-denotation").unwrap(),
        },
        ProjectionFact::Transformed {
            obligation: ProjectionText::new("numeric-representation").unwrap(),
            law: ProjectionText::new("finite-binary64-operation-order@1").unwrap(),
        },
    ];
    assert!(!Domain.validate(
        &p,
        Some(&p.target()),
        &false_exact,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
}
#[test]
fn program_storage_work_range_and_rounded_denominator_refuse_before_execution() {
    assert_eq!(
        prepare(&[], Mode::NearestTiesEven, Q::ONE),
        Err(Refusal::InvalidProgram)
    );
    assert_eq!(
        prepare(&[Op::Negate { input: 0 }], Mode::NearestTiesEven, Q::ONE),
        Err(Refusal::InvalidProgram)
    );
    assert_eq!(
        prepare(
            &vec![Op::Constant(Box::new(Q::ONE)); 33],
            Mode::NearestTiesEven,
            Q::ONE
        ),
        Err(Refusal::StepLimit)
    );
    assert_eq!(
        Prepared::prepare(
            &[Op::Constant(Box::new(Q::ONE))],
            Mode::NearestTiesEven,
            Q::ONE,
            Limits {
                maximum_steps: 32,
                maximum_retained_bytes: 0
            },
            &mut work()
        ),
        Err(Refusal::StorageLimit)
    );
    assert_eq!(
        Prepared::prepare(
            &[Op::Constant(Box::new(Q::ONE))],
            Mode::NearestTiesEven,
            Q::ONE,
            limits(),
            &mut Work::new(0)
        ),
        Err(Refusal::Projection(Binary64Refusal::Numeric(
            ExactNumericRefusal::WorkLimit
        )))
    );
    let huge = Q::from_binary64(IeeeF64::from_value(f64::MAX), &mut work()).unwrap();
    assert_eq!(
        prepare(
            &[
                Op::Constant(Box::new(huge)),
                Op::Multiply { left: 0, right: 0 }
            ],
            Mode::NearestTiesEven,
            Q::ONE
        ),
        Err(Refusal::Projection(Binary64Refusal::Numeric(
            ExactNumericRefusal::OutOfRange
        )))
    );
    let tiny = Q::from_i128(2).checked_power(-1075, &mut work()).unwrap();
    assert_eq!(
        prepare(
            &[
                Op::Constant(Box::new(Q::ONE)),
                Op::Constant(Box::new(tiny)),
                Op::Divide { left: 0, right: 1 }
            ],
            Mode::NearestTiesEven,
            Q::ONE
        ),
        Err(Refusal::RoundedDivisionByZero)
    );
}

#[test]
fn finite_software_operations_match_native_nearest_even_for_independent_ieee_inputs() {
    let tolerance = Q::from_i128(2).checked_power(971, &mut work()).unwrap();
    let mut state = 0x4abf21cc8765ad01u64;
    for _ in 0..64 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let left_bits = (state & ((1 << 52) - 1))
            | (((700 + (state >> 52) % 600) & 0x7ff) << 52)
            | (state & (1 << 63));
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let right_bits = (state & ((1 << 52) - 1)) | (1023 << 52) | (state & (1 << 63));
        let left = IeeeF64::from_bits(left_bits);
        let right = IeeeF64::from_bits(right_bits);
        for (operation, expected) in [
            (Op::Add { left: 0, right: 1 }, left.value() + right.value()),
            (
                Op::Subtract { left: 0, right: 1 },
                left.value() - right.value(),
            ),
            (
                Op::Multiply { left: 0, right: 1 },
                left.value() * right.value(),
            ),
            (
                Op::Divide { left: 0, right: 1 },
                left.value() / right.value(),
            ),
        ] {
            let result = prepare(
                &[
                    Op::IeeeConstant(left),
                    Op::IeeeConstant(right),
                    operation.clone(),
                ],
                Mode::NearestTiesEven,
                tolerance,
            )
            .unwrap();
            assert_eq!(
                result.target().bits(),
                expected.to_bits(),
                "{left_bits:x}, {right_bits:x}, {operation:?}"
            );
        }
    }
}
