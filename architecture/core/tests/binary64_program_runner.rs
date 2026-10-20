//! A prepared program is not an executed certificate or a fabricated input.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;

fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn limits() -> Binary64ProgramRunnerLimits {
    Binary64ProgramRunnerLimits {
        algorithm: Binary64AlgorithmLimits {
            maximum_steps: 32,
            maximum_retained_bytes: 1_000_000,
        },
        maximum_retained_payload_bytes: 3_000_000,
        maximum_execution_work: 100_000_000,
    }
}
fn program(input: i128) -> [Binary64Operation; 4] {
    [
        Binary64Operation::Constant(Box::new(ExactRational::from_i128(input))),
        Binary64Operation::Constant(Box::new(ExactRational::ONE)),
        Binary64Operation::Subtract { left: 0, right: 1 },
        Binary64Operation::Divide { left: 1, right: 2 },
    ]
}

#[test]
fn singular_placeholder_never_becomes_a_certificate_and_real_inputs_recover_without_growth() {
    for rounding in [
        Binary64Rounding::NearestTiesEven,
        Binary64Rounding::TowardZero,
        Binary64Rounding::TowardPositive,
        Binary64Rounding::TowardNegative,
    ] {
        assert!(PreparedBinary64Algorithm::prepare(
            &program(1),
            rounding,
            ExactRational::ZERO,
            limits().algorithm,
            &mut work()
        )
        .is_err());
        let mut runner = PreparedBinary64ProgramRunner::new(
            &program(1),
            &[0],
            rounding,
            ExactRational::ZERO,
            limits(),
            &mut work(),
        )
        .unwrap();
        assert!(runner.last_success().is_none());
        let retained = runner.retained_payload_bytes();
        let expected = PreparedBinary64Algorithm::prepare(
            &program(2),
            rounding,
            ExactRational::ZERO,
            limits().algorithm,
            &mut work(),
        )
        .unwrap();
        let recovered = PreparedBinary64Algorithm::prepare(
            &program(3),
            rounding,
            ExactRational::ZERO,
            limits().algorithm,
            &mut work(),
        )
        .unwrap();
        let one = ExactRational::ONE;
        let two = ExactRational::from_i128(2);
        let three = ExactRational::from_i128(3);
        let four = ExactRational::from_i128(4);
        let (correct, allocations) = observe(|| {
            let mut failed_work = work();
            let initial_refusal = matches!(
                runner.execute(&[one], &mut failed_work),
                Err(Binary64AlgorithmRefusal::Projection(
                    Binary64Refusal::Numeric(ExactNumericRefusal::DivisionByZero)
                ))
            ) && failed_work.spent() > 0
                && runner.last_success().is_none();
            let first_success = runner
                .execute(&[two], &mut work())
                .is_ok_and(|actual| actual == &expected);
            let mut tolerance_work = work();
            let tolerance_refusal = matches!(
                runner.execute(&[four], &mut tolerance_work),
                Err(Binary64AlgorithmRefusal::Projection(
                    Binary64Refusal::ToleranceExceeded
                ))
            ) && tolerance_work.spent() > 0
                && runner.last_success() == Some(&expected);
            initial_refusal
                && first_success
                && tolerance_refusal
                && runner
                    .execute(&[three], &mut work())
                    .is_ok_and(|actual| actual == &recovered)
        });
        assert!(correct);
        assert_eq!(allocations, 0);
        assert_eq!(runner.retained_payload_bytes(), retained);
        assert_eq!(
            runner.maximum_execution_work(),
            limits().maximum_execution_work
        );
    }
}

#[test]
fn structure_storage_and_execution_work_are_independently_bounded() {
    let operations = program(1);
    let make = |ops: &[Binary64Operation], slots: &[u8], bounds| {
        PreparedBinary64ProgramRunner::new(
            ops,
            slots,
            Binary64Rounding::NearestTiesEven,
            ExactRational::ZERO,
            bounds,
            &mut work(),
        )
    };
    for slots in [&[0, 0][..], &[4][..], &[2][..]] {
        assert!(matches!(
            make(&operations, slots, limits()),
            Err(Binary64AlgorithmRefusal::InvalidProgram)
        ));
    }
    assert!(matches!(
        make(&[], &[], limits()),
        Err(Binary64AlgorithmRefusal::InvalidProgram)
    ));
    assert!(matches!(
        make(
            &[Binary64Operation::Add { left: 0, right: 0 }],
            &[],
            limits()
        ),
        Err(Binary64AlgorithmRefusal::InvalidProgram)
    ));
    let normal = make(&operations, &[0], limits()).unwrap();
    let mut small = limits();
    small.maximum_retained_payload_bytes = normal.retained_payload_bytes() as u32 - 1;
    let (refused, allocations) = observe(|| {
        matches!(
            make(&operations, &[0], small),
            Err(Binary64AlgorithmRefusal::StorageLimit)
        )
    });
    assert!(refused);
    assert_eq!(allocations, 0);
    let mut short = limits();
    short.maximum_execution_work = 1;
    let mut runner = make(&operations, &[0], short).unwrap();
    let mut charged = work();
    assert!(matches!(
        runner.execute(&[ExactRational::from_i128(2)], &mut charged),
        Err(Binary64AlgorithmRefusal::Projection(
            Binary64Refusal::Numeric(ExactNumericRefusal::WorkLimit)
        ))
    ));
    assert_eq!(charged.spent(), 1);
    assert!(runner.last_success().is_none());
}

#[test]
fn projected_slots_keep_their_admitted_ceiling_without_an_initial_certificate() {
    let ceiling = ExactRational::ratio(1, 1000, &mut work()).unwrap();
    let input = |source, target, maximum_error| {
        Binary64ProjectedInput::new(
            ExactRational::from_i128(source),
            IeeeF64::from_bits(target),
            maximum_error,
            &mut work(),
        )
        .unwrap()
    };
    let mut operations = program(1);
    operations[0] =
        Binary64Operation::ProjectedConstant(Box::new(input(1, 1f64.to_bits(), ceiling)));
    let mut runner = PreparedBinary64ProgramRunner::new(
        &operations,
        &[0],
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        limits(),
        &mut work(),
    )
    .unwrap();
    let bad = input(1, 2f64.to_bits(), ExactRational::from_i128(2));
    let good = input(3, 3f64.to_bits(), ExactRational::ZERO);
    let retained = runner.retained_payload_bytes();
    let (correct, allocations) = observe(|| {
        let mut charged = work();
        let refused = matches!(
            runner.execute_inputs(&[Binary64AlgorithmInput::Projected(&bad)], &mut charged),
            Err(Binary64AlgorithmRefusal::Projection(
                Binary64Refusal::ToleranceExceeded
            ))
        ) && charged.spent() > 0
            && runner.last_success().is_none();
        refused && runner.execute_inputs(&[Binary64AlgorithmInput::Projected(&good)], &mut work())
            .is_ok_and(|p| p.target().bits() == 0.5f64.to_bits()
                && matches!(p.steps()[0].operation(), Binary64Operation::ProjectedConstant(i) if i.maximum_error() == ceiling))
    });
    assert!(correct);
    assert_eq!(allocations, 0);
    assert_eq!(runner.retained_payload_bytes(), retained);
    assert!(runner.last_success().is_some());
}
