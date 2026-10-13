use conduit_core::{
    Binary64Rounding as Mode, ExactNumericWork as Work, ExactRational as Q,
    MathematicalEnclosureRequirements as Requirements, MathematicalValue as Value,
    PreparedMathematicalEnclosure as Enclosure, PreparedSymbolicBinary64 as Prepared,
    SymbolicBinary64Refusal as Refusal,
};
fn work() -> Work {
    Work::new(100_000_000)
}
fn q(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn enclosure(name: &str, width: Q) -> Enclosure {
    enclose(&Value::parse_literal(name, &mut work()).unwrap(), width)
}
fn enclose(source: &Value, width: Q) -> Enclosure {
    Enclosure::prepare(
        source,
        Requirements {
            maximum_width: width,
            maximum_leaf_width: q(1, 100_000_000_000_000_000_000),
            maximum_constant_terms: 32,
            maximum_root_refinements: 96,
            maximum_scratch_payload_bytes: 131072,
            maximum_retained_payload_bytes: 131072,
        },
        &mut work(),
    )
    .unwrap()
}
#[test]
fn constants_keep_exact_source_and_prove_correct_rounding() {
    for (name, bits) in [("pi", 0x400921fb54442d18), ("tau", 0x401921fb54442d18)] {
        let bound = enclosure(name, q(1, 10_000_000_000_000_000_000));
        let source = bound.source().clone();
        let result = Prepared::prepare(
            bound,
            Mode::NearestTiesEven,
            q(1, 1_000_000_000_000_000),
            &mut work(),
        )
        .unwrap();
        assert_eq!(result.target().bits(), bits);
        assert_eq!(result.enclosure().source(), &source);
        assert!(result.spent_limb_work() > 0);
        assert!(Prepared::prepare(
            result.enclosure().clone(),
            Mode::NearestTiesEven,
            Q::ZERO,
            &mut work()
        )
        .is_err());
    }
}
#[test]
fn broad_enclosure_refuses_to_guess_rounding() {
    let source = Value::parse_literal("pi", &mut work()).unwrap();
    let requirements = Requirements {
        maximum_width: q(1, 100),
        maximum_leaf_width: q(1, 100),
        maximum_constant_terms: 32,
        maximum_root_refinements: 96,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let broad = Enclosure::prepare(&source, requirements, &mut work()).unwrap();
    assert_eq!(
        Prepared::prepare(broad, Mode::NearestTiesEven, Q::ONE, &mut work()),
        Err(Refusal::RoundingUndetermined)
    );
}

#[test]
fn roots_and_negative_constants_round_in_each_direction() {
    use conduit_core::{MathematicalNode as Node, MathematicalValueBuilder as Builder};
    let mut b = Builder::new();
    let two = b
        .insert(Node::Rational(Q::from_i128(2)), &mut work())
        .unwrap();
    let root = b
        .insert(Node::PrincipalSquareRoot(two), &mut work())
        .unwrap();
    let sqrt_two = b.finish(root, &mut work()).unwrap();
    let mut b = Builder::new();
    let pi = b.insert(Node::Pi, &mut work()).unwrap();
    let negative = b.insert(Node::Negate(pi), &mut work()).unwrap();
    let negative_pi = b.finish(negative, &mut work()).unwrap();
    for (source, expected) in [
        (
            sqrt_two,
            [
                0x3ff6a09e667f3bcd,
                0x3ff6a09e667f3bcc,
                0x3ff6a09e667f3bcd,
                0x3ff6a09e667f3bcc,
            ],
        ),
        (
            negative_pi,
            [
                0xc00921fb54442d18,
                0xc00921fb54442d18,
                0xc00921fb54442d18,
                0xc00921fb54442d19,
            ],
        ),
    ] {
        let bound = enclose(&source, q(1, 10_000_000_000_000_000_000));
        for (mode, bits) in [
            Mode::NearestTiesEven,
            Mode::TowardZero,
            Mode::TowardPositive,
            Mode::TowardNegative,
        ]
        .into_iter()
        .zip(expected)
        {
            let result = Prepared::prepare(
                bound.clone(),
                mode,
                q(1, 1_000_000_000_000_000),
                &mut work(),
            )
            .unwrap();
            assert_eq!(result.target().bits(), bits);
            assert_eq!(result.enclosure().source(), &source);
            assert_eq!(result.rounding(), mode);
        }
    }
}
#[test]
fn bounded_work_and_negative_tolerance_refuse() {
    use conduit_core::{Binary64Refusal, ExactNumericRefusal};
    let bound = enclosure("pi", q(1, 10_000_000_000_000_000_000));
    let mut exhausted = Work::new(0);
    assert_eq!(
        Prepared::prepare(bound.clone(), Mode::NearestTiesEven, Q::ONE, &mut exhausted),
        Err(Refusal::Projection(Binary64Refusal::Numeric(
            ExactNumericRefusal::WorkLimit
        )))
    );
    assert_eq!(
        Prepared::prepare(bound, Mode::NearestTiesEven, Q::ONE.negated(), &mut work()),
        Err(Refusal::Projection(Binary64Refusal::NegativeTolerance))
    );
}

#[test]
fn fidelity_refuses_false_exactness_and_unrequested_rounding() {
    use conduit_core::{
        projection::*, IeeeF64, SymbolicBinary64Domain as Domain, SymbolicBinary64Policy as Policy,
    };
    let tolerance = q(1, 1_000_000_000_000_000);
    let result = Prepared::prepare(
        enclosure("pi", q(1, 10_000_000_000_000_000_000)),
        Mode::NearestTiesEven,
        tolerance,
        &mut work(),
    )
    .unwrap();
    let facts = result.projection_facts();
    assert!(Domain.validate(
        &result,
        Some(&result.target()),
        &facts,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
    assert!(Policy {
        maximum_error: tolerance,
        rounding: Mode::NearestTiesEven
    }
    .permits(&result, &result.target(), &facts));
    assert!(!Policy {
        maximum_error: Q::ZERO,
        rounding: Mode::NearestTiesEven
    }
    .permits(&result, &result.target(), &facts));
    assert!(!Policy {
        maximum_error: tolerance,
        rounding: Mode::TowardPositive
    }
    .permits(&result, &result.target(), &facts));
    assert!(!Domain.validate(
        &result,
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
            law: ProjectionText::new("certified-symbolic-binary64@1").unwrap(),
        },
    ];
    assert!(!Domain.validate(
        &result,
        Some(&result.target()),
        &false_exact,
        &[],
        &[],
        ProjectionMechanism::Completed
    ));
}
