use conduit_core::*;
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn requirements() -> MathematicalEnclosureRequirements {
    MathematicalEnclosureRequirements {
        maximum_width: q(1, 1_000_000_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
fn limits() -> Binary64AlgorithmLimits {
    Binary64AlgorithmLimits {
        maximum_steps: 1,
        maximum_retained_bytes: 131072,
    }
}
#[test]
fn angle_pi_source_midpoint_and_machine_rounding_remain_distinct() {
    let source = PreparedAngleReferenceConversion::from_checked(
        Quantity::new(1, Unit::Turn),
        Unit::Radian,
        "1turn",
        "rad",
        &mut work(),
    )
    .unwrap();
    let tolerance = q(1, 1_000_000_000_000_000);
    let prepared = PreparedEnclosedBinary64::prepare(
        source.denotation(),
        requirements(),
        Binary64Rounding::NearestTiesEven,
        tolerance,
        limits(),
        &mut work(),
    )
    .unwrap();
    assert_eq!(prepared.source(), source.denotation());
    assert_eq!(prepared.source().as_rational(), None);
    assert_eq!(
        prepared.rounded().target().bits(),
        std::f64::consts::TAU.to_bits()
    );
    assert_eq!(
        prepared
            .maximum_error()
            .checked_compare(tolerance, &mut work())
            .unwrap(),
        Ordering::Less
    );
    assert_ne!(prepared.enclosure().width(), ExactRational::ZERO);
    // Independently check the total conservative bound against both endpoints.
    let target = ExactRational::from_binary64(prepared.rounded().target(), &mut work()).unwrap();
    for endpoint in [
        prepared.enclosure().target().lower(),
        prepared.enclosure().target().upper(),
    ] {
        let distance = target.checked_subtract(endpoint, &mut work()).unwrap();
        let distance = if distance.is_negative() {
            distance.negated()
        } else {
            distance
        };
        assert_ne!(
            distance
                .checked_compare(prepared.maximum_error(), &mut work())
                .unwrap(),
            Ordering::Greater
        );
    }
    assert!(prepared.retained_payload_bytes() >= core::mem::size_of_val(&prepared));
}
#[test]
fn unsupported_precision_and_finite_work_refuse_during_preparation() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    assert!(PreparedEnclosedBinary64::prepare(
        &pi,
        requirements(),
        Binary64Rounding::NearestTiesEven,
        ExactRational::ZERO,
        limits(),
        &mut work()
    )
    .is_err());
    assert!(matches!(
        PreparedEnclosedBinary64::prepare(
            &pi,
            MathematicalEnclosureRequirements {
                maximum_constant_terms: 0,
                ..requirements()
            },
            Binary64Rounding::NearestTiesEven,
            ExactRational::ONE,
            limits(),
            &mut work()
        ),
        Err(EnclosedBinary64Refusal::Enclosure(_))
    ));
    assert!(PreparedEnclosedBinary64::prepare(
        &pi,
        requirements(),
        Binary64Rounding::NearestTiesEven,
        ExactRational::ONE,
        limits(),
        &mut ExactNumericWork::new(0)
    )
    .is_err());
}
#[test]
fn an_exact_representable_point_needs_no_invented_error() {
    let source = MathematicalValue::rational(ExactRational::ONE);
    let prepared = PreparedEnclosedBinary64::prepare(
        &source,
        requirements(),
        Binary64Rounding::TowardNegative,
        ExactRational::ZERO,
        limits(),
        &mut work(),
    )
    .unwrap();
    assert_eq!(prepared.maximum_error(), ExactRational::ZERO);
    assert_eq!(prepared.rounded().target().bits(), 1_f64.to_bits());
}
