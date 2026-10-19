//! Dynamic exact inputs reuse the existing operation-order engine and proof.
use conduit_core::{
    Binary64AlgorithmLimits as Limits, Binary64AlgorithmRefusal as Refusal,
    Binary64ErrorClaim as Claim, Binary64Operation as Op, Binary64Refusal,
    Binary64Rounding as Mode, ExactNumericRefusal, ExactNumericWork as Work, ExactRational as Q,
    IeeeF64, PreparedBinary64Algorithm as Prepared, PreparedBinary64AlgorithmRunner as Runner,
    PreparedBinary64Projection as Point,
};
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
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
fn prepare(p: &[Op], mode: Mode, tolerance: Q) -> Prepared {
    Prepared::prepare(p, mode, tolerance, limits(), &mut work()).unwrap()
}
fn program(v: [Q; 3]) -> [Op; 5] {
    [
        Op::Constant(Box::new(v[0])),
        Op::Constant(Box::new(v[1])),
        Op::Constant(Box::new(v[2])),
        Op::Add { left: 0, right: 1 },
        Op::Subtract { left: 3, right: 2 },
    ]
}

#[test]
fn changing_inputs_match_complete_fresh_certificates_in_every_rounding_without_allocation() {
    let cases = [
        [q("0.1"), q("0.2"), q("0.3")],
        [q("9007199254740992"), Q::ONE, q("9007199254740992")],
        [Q::from_i128(3), Q::from_i128(4), Q::from_i128(5)],
    ];
    let baseline_program = program(cases[0]);
    let (baseline, allocations) =
        observe(|| prepare(&baseline_program, Mode::NearestTiesEven, Q::from_i128(2)));
    assert!(allocations > 0);
    println!(
        "COMPOSITE_FRESH_PREPARATION allocations={allocations} steps={}",
        baseline.steps().len()
    );
    for mode in [
        Mode::NearestTiesEven,
        Mode::TowardZero,
        Mode::TowardPositive,
        Mode::TowardNegative,
    ] {
        let template = prepare(&program(cases[2]), mode, Q::from_i128(2));
        let mut runner = Runner::new(&template, &[0, 1, 2], 3_000_000).unwrap();
        let capacity = runner.retained_payload_bytes();
        for inputs in cases {
            let expected = prepare(&program(inputs), mode, Q::from_i128(2));
            let (matches, allocations) = observe(|| {
                (0..8).all(|_| {
                    runner
                        .execute(&inputs, &mut work())
                        .is_ok_and(|p| p == &expected)
                })
            });
            assert!(matches);
            assert_eq!(allocations, 0);
            assert_eq!(runner.retained_payload_bytes(), capacity);
        }
    }
}

#[test]
fn accumulated_error_refuses_transactionally_then_recovers_without_allocation() {
    let program = [
        Op::Constant(Box::new(Q::ZERO)),
        Op::Constant(Box::new(Q::ZERO)),
        Op::Subtract { left: 0, right: 1 },
    ];
    let template = prepare(&program, Mode::NearestTiesEven, Q::ZERO);
    let mut runner = Runner::new(&template, &[0, 1], 3_000_000).unwrap();
    let valid = [Q::from_i128(3), Q::ONE];
    runner.execute(&valid, &mut work()).unwrap();
    let before = runner.last_success().clone();
    let loss = [q("9007199254740993"), q("9007199254740992")];
    let (correct, allocations) = observe(|| {
        (0..16).all(|_| {
            let mut failed_work = work();
            let refused = matches!(
                runner.execute(&loss, &mut failed_work),
                Err(Refusal::Projection(Binary64Refusal::ToleranceExceeded))
            );
            refused
                && failed_work.spent() > 0
                && runner.last_success() == &before
                && runner
                    .execute(&valid, &mut work())
                    .is_ok_and(|p| p == &before)
        })
    });
    assert!(correct);
    assert_eq!(allocations, 0);

    let loose = prepare(&program, Mode::NearestTiesEven, Q::ONE);
    let mut loose = Runner::new(&loose, &[0, 1], 3_000_000).unwrap();
    let computed = loose.execute(&loss, &mut work()).unwrap();
    assert_eq!(computed.source(), Q::ONE);
    assert_eq!(computed.target().bits(), 0);
    assert_eq!(computed.error(), Claim::ExactDistance(Q::ONE));
    let final_only = Point::prepare(Q::ONE, Mode::NearestTiesEven, Q::ZERO, &mut work()).unwrap();
    assert_eq!(final_only.target().bits(), 1f64.to_bits());
    assert_ne!(computed.target(), final_only.target());
}

#[test]
fn slots_and_double_buffer_storage_are_admitted_before_execution() {
    let template = prepare(&program([Q::ONE; 3]), Mode::NearestTiesEven, Q::ONE);
    for slots in [&[0, 0][..], &[3][..], &[255][..]] {
        assert!(matches!(
            Runner::new(&template, slots, 3_000_000),
            Err(Refusal::InvalidProgram)
        ));
    }
    assert!(matches!(
        Runner::new(&template, &[0; 33], 3_000_000),
        Err(Refusal::StepLimit)
    ));
    let size = Runner::new(&template, &[0], 3_000_000)
        .unwrap()
        .retained_payload_bytes() as u32;
    assert!(Runner::new(&template, &[0], size).is_ok());
    assert!(matches!(
        Runner::new(&template, &[0], size - 1),
        Err(Refusal::StorageLimit)
    ));
    let ieee = prepare(
        &[Op::IeeeConstant(IeeeF64::from_bits(0))],
        Mode::NearestTiesEven,
        Q::ZERO,
    );
    assert!(matches!(
        Runner::new(&ieee, &[0], 3_000_000),
        Err(Refusal::InvalidProgram)
    ));
}

#[test]
fn work_and_input_refusals_preserve_the_last_certificate_and_allow_recovery() {
    let template = prepare(&program([Q::ONE; 3]), Mode::NearestTiesEven, Q::ZERO);
    let mut runner = Runner::new(&template, &[0, 1, 2], 3_000_000).unwrap();
    let before = runner.last_success().clone();
    let mut small = Work::new(1);
    assert!(matches!(
        runner.execute(&[Q::ONE; 3], &mut small),
        Err(Refusal::Projection(Binary64Refusal::Numeric(
            ExactNumericRefusal::WorkLimit
        )))
    ));
    assert_eq!(small.spent(), 1);
    assert_eq!(runner.last_success(), &before);
    let mut budget = work();
    assert!(matches!(
        runner.execute(&[], &mut budget),
        Err(Refusal::InvalidProgram)
    ));
    assert_eq!(budget.spent(), 0);
    assert_eq!(runner.last_success(), &before);
    assert_eq!(runner.execute(&[Q::ONE; 3], &mut work()).unwrap(), &before);
    let mut oversized = Work::new(u64::MAX);
    let result = runner.execute(&[Q::ONE; 3], &mut oversized).unwrap();
    assert_eq!(result.admitted_limb_work(), template.admitted_limb_work());
    assert_eq!(oversized.spent(), result.spent_limb_work());
}

#[test]
fn nonprefix_slot_order_preserves_unbound_literals_and_operation_order() {
    let template = prepare(
        &program([Q::ONE, Q::from_i128(2), Q::from_i128(3)]),
        Mode::NearestTiesEven,
        Q::ZERO,
    );
    let mut runner = Runner::new(&template, &[2, 0], 3_000_000).unwrap();
    let expected = prepare(
        &program([Q::from_i128(7), Q::from_i128(2), Q::from_i128(5)]),
        Mode::NearestTiesEven,
        Q::ZERO,
    );
    let (correct, allocations) = observe(|| {
        runner
            .execute(&[Q::from_i128(5), Q::from_i128(7)], &mut work())
            .is_ok_and(|p| {
                p == &expected
                    && p.source() == Q::from_i128(4)
                    && p.target().bits() == 4f64.to_bits()
            })
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}

#[test]
fn rounded_zero_denominator_refuses_after_partial_work_then_valid_division_recovers() {
    let p = [
        Op::Constant(Box::new(Q::ONE)),
        Op::Constant(Box::new(Q::ONE)),
        Op::Divide { left: 0, right: 1 },
    ];
    let template = prepare(&p, Mode::NearestTiesEven, Q::ONE);
    let mut runner = Runner::new(&template, &[0, 1], 3_000_000).unwrap();
    let before = runner.last_success().clone();
    let tiny = Q::from_i128(2).checked_power(-1075, &mut work()).unwrap();
    let half = Q::ratio(1, 2, &mut work()).unwrap();
    let (correct, allocations) = observe(|| {
        let mut spent = work();
        matches!(
            runner.execute(&[Q::ONE, tiny], &mut spent),
            Err(Refusal::RoundedDivisionByZero)
        ) && spent.spent() > 0
            && runner.last_success() == &before
            && runner
                .execute(&[Q::ONE, Q::from_i128(2)], &mut work())
                .is_ok_and(|p| {
                    p.source() == half
                        && p.target().bits() == 0.5f64.to_bits()
                        && p.error() == Claim::ExactDistance(Q::ZERO)
                })
    });
    assert!(correct);
    assert_eq!(allocations, 0);
}
