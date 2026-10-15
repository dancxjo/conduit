use conduit_core::{
    ExactNumericWork as Work, ExactRational as Q, MathematicalComparison as C,
    MathematicalEnclosureRefusal as EnclosureRefusal,
    MathematicalEnclosureRequirements as Requirements, MathematicalNode as N, MathematicalNodeId,
    MathematicalValue as V, MathematicalValueBuilder as B, MathematicalValueRefusal as R,
    MathematicalValueView as View, PreparedMathematicalEnclosure as Prepared,
    MATHEMATICAL_VALUE_MAX_ENCODED_BYTES, MATHEMATICAL_VALUE_VALIDATION_WORK,
};
fn work() -> Work {
    Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK)
}
fn denote(q: Q, operation: fn(MathematicalNodeId) -> N) -> Result<V, R> {
    let mut builder = B::new();
    let mut w = work();
    let input = builder.insert(N::Rational(q), &mut w)?;
    let root = builder.insert(operation(input), &mut w)?;
    builder.finish(root, &mut w)
}
fn bytes(value: &V) -> Vec<u8> {
    let mut output = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = value.encode(&mut output).unwrap();
    output.truncate(length);
    output
}
#[test]
fn cardinal_turns_are_exact_and_tangent_poles_are_named() {
    for quarter in -9i128..=9 {
        let q = Q::ratio(quarter, 4, &mut work()).unwrap();
        let i = usize::try_from(quarter.rem_euclid(4)).unwrap();
        assert_eq!(
            denote(q, N::SineTurns).unwrap(),
            V::rational(Q::from_i128([0, 1, 0, -1][i]))
        );
        assert_eq!(
            denote(q, N::CosineTurns).unwrap(),
            V::rational(Q::from_i128([1, 0, -1, 0][i]))
        );
        if i % 2 == 1 {
            assert_eq!(denote(q, N::TangentTurns), Err(R::TrigonometricPole));
        } else {
            assert_eq!(denote(q, N::TangentTurns).unwrap(), V::rational(Q::ZERO));
        }
    }
}
#[test]
fn non_cardinal_denotation_survives_borrowed_codec_without_invented_enclosure() {
    for operation in [N::SineTurns, N::CosineTurns, N::TangentTurns] {
        let value = denote(Q::ratio(1, 7, &mut work()).unwrap(), operation).unwrap();
        assert_eq!(
            value,
            denote(Q::ratio(8, 7, &mut work()).unwrap(), operation).unwrap()
        );
        let encoded = bytes(&value);
        let view = View::decode(&encoded, &mut work()).unwrap();
        assert_eq!(view.node_count(), 2);
        assert_eq!(view.as_rational(&mut work()).unwrap(), None);
        assert_eq!(V::decode(&encoded, &mut work()).unwrap(), value);
        assert_eq!(
            value.compare(&V::rational(Q::ZERO), &mut work()),
            Ok(C::Unknown)
        );
        let width = Q::ratio(1, 100, &mut work()).unwrap();
        let requirements = Requirements {
            maximum_width: width,
            maximum_leaf_width: width,
            maximum_constant_terms: 32,
            maximum_root_refinements: 64,
            maximum_scratch_payload_bytes: 100_000,
            maximum_retained_payload_bytes: 100_000,
        };
        assert!(matches!(
            Prepared::prepare(&value, requirements, &mut work()),
            Err(EnclosureRefusal::UnsupportedTrigonometricEnclosure)
        ));
    }
}
#[test]
fn huge_whole_turns_reduce_before_narrowing_and_unproved_tangent_refuses() {
    let mut w = work();
    let whole = Q::from_decimal_parts(1, 1000, &mut w).unwrap();
    let quarter = Q::ratio(1, 4, &mut w).unwrap();
    for base in [whole, whole.negated()] {
        let value = base.checked_add(quarter, &mut w).unwrap();
        assert_eq!(denote(value, N::SineTurns).unwrap(), V::rational(Q::ONE));
    }
    let mut b = B::new();
    let pi = b.insert(N::Pi, &mut work()).unwrap();
    assert_eq!(
        b.insert(N::TangentTurns(pi), &mut work()),
        Err(R::UnsupportedDomainProof)
    );
    let sine = b.insert(N::SineTurns(pi), &mut work()).unwrap();
    let value = b.finish(sine, &mut work()).unwrap();
    assert_eq!(V::decode(&bytes(&value), &mut work()).unwrap(), value);
}

#[test]
fn typed_angle_unit_and_numeric_projections_retain_separate_evidence() {
    use conduit_core::{
        AngleTrigonometricFunction as F, PreparedAngleTrigonometry as T, Quantity, Unit,
    };
    let mut results = Vec::new();
    for original in ["0.25turn", "90°"] {
        let source = Quantity::parse_plot_literal(original).unwrap();
        let prepared = T::from_checked(F::Sine, source, original, &mut work()).unwrap();
        assert_eq!(prepared.coordinate_projection().original(), original);
        assert_eq!(prepared.coordinate_projection().source(), source);
        let canonical = prepared.coordinate_projection().result().unwrap();
        assert_eq!(canonical.unit(), Unit::Turn);
        assert_eq!(canonical.coordinate(), Q::ratio(1, 4, &mut work()).unwrap());
        assert_eq!(prepared.denotation(), &V::rational(Q::ONE));
        let limits = Requirements {
            maximum_width: Q::ZERO,
            maximum_leaf_width: Q::ZERO,
            maximum_constant_terms: 1,
            maximum_root_refinements: 1,
            maximum_scratch_payload_bytes: 100_000,
            maximum_retained_payload_bytes: 100_000,
        };
        let numeric = Prepared::prepare(prepared.denotation(), limits, &mut work()).unwrap();
        assert_eq!(numeric.source(), prepared.denotation());
        assert_eq!(numeric.target().lower(), Q::ONE);
        assert_eq!(numeric.target().upper(), Q::ONE);
        results.push(prepared.denotation().clone());
    }
    assert_eq!(results[0], results[1]);
}
#[test]
fn wrong_dimension_forged_source_and_tangent_domain_are_distinct() {
    use conduit_core::{
        AngleTrigonometricFunction as F, AngleTrigonometryRefusal as Error,
        PreparedAngleTrigonometry as T, Quantity,
    };
    assert_eq!(
        T::from_checked(
            F::Sine,
            Quantity::parse_plot_literal("1m").unwrap(),
            "1m",
            &mut work()
        ),
        Err(Error::NotAngle)
    );
    let quarter = Quantity::parse_plot_literal("0.25turn").unwrap();
    assert!(matches!(
        T::from_checked(F::Sine, quarter, "90°", &mut work()),
        Err(Error::SourceEvidence(_))
    ));
    assert_eq!(
        T::from_checked(F::Tangent, quarter, "0.25turn", &mut work()),
        Err(Error::Denotation(R::TrigonometricPole))
    );
    let radian = Quantity::parse_plot_literal("1rad").unwrap();
    assert!(matches!(
        T::from_checked(F::Sine, radian, "1rad", &mut work()),
        Err(Error::Coordinate(_))
    ));
}

#[test]
fn valid_wide_non_cardinal_fraction_does_not_overflow_cardinal_detection() {
    let mut w = Work::new(200_000_000);
    let power = Q::from_i128(2).checked_power(4095, &mut w).unwrap();
    let numerator = power.checked_subtract(Q::from_i128(2), &mut w).unwrap();
    let denominator = power.checked_subtract(Q::ONE, &mut w).unwrap();
    let fraction = numerator.checked_divide(denominator, &mut w).unwrap();
    let value = denote(fraction, N::SineTurns).unwrap();
    let encoded = bytes(&value);
    assert_eq!(V::decode(&encoded, &mut work()).unwrap(), value);
}

#[test]
fn admitted_storage_writer_matches_prepared_denotation_and_refuses_bounds() {
    use conduit_core::{
        encode_angle_trigonometric_denotation as write, AngleTrigonometricFunction as F,
        PreparedAngleTrigonometry as T, Quantity,
    };
    let mut output = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    for original in [
        "0turn",
        "0.25turn",
        "90°",
        "-810°",
        "0.125turn",
        "0.17turn",
        "1000000000000000000.25turn",
    ] {
        let source = Quantity::parse_plot_literal(original).unwrap();
        for function in [F::Sine, F::Cosine, F::Tangent] {
            let prepared = T::from_checked(function, source, original, &mut work());
            let written = write(function, source, &mut output, &mut work());
            match (prepared, written) {
                (Ok(value), Ok(length)) => assert_eq!(&output[..length], bytes(value.denotation())),
                (Err(a), Err(b)) => assert_eq!(a, b),
                _ => panic!("writer/preparation disagreement"),
            }
        }
    }
    let source = Quantity::parse_plot_literal("0.17turn").unwrap();
    assert!(write(F::Sine, source, &mut [0; 1], &mut work()).is_err());
    assert!(write(F::Sine, source, &mut output, &mut Work::new(0)).is_err());
}

#[test]
fn borrowed_codec_rejects_noncanonical_trig_graphs_and_self_references() {
    fn raw(q: Q, tag: u8) -> Vec<u8> {
        let mut rational = vec![0; conduit_core::EXACT_RATIONAL_MAX_ENCODED_BYTES];
        let length = q.encode(&mut rational).unwrap();
        rational.truncate(length);
        let mut output = vec![1, 1, 2, 0];
        output.extend_from_slice(&(length as u16).to_le_bytes());
        output.extend_from_slice(&rational);
        output.extend_from_slice(&[tag, 0]);
        output
    }
    let mut bytes = raw(Q::ratio(1, 7, &mut work()).unwrap(), 9);
    assert!(View::decode(&bytes, &mut work()).is_ok());
    *bytes.last_mut().unwrap() = 1;
    assert!(matches!(
        View::decode(&bytes, &mut work()),
        Err(R::InvalidReference)
    ));
    for (q, tag) in [
        (Q::ratio(1, 4, &mut work()).unwrap(), 9),
        (Q::ratio(8, 7, &mut work()).unwrap(), 10),
    ] {
        assert!(matches!(
            View::decode(&raw(q, tag), &mut work()),
            Err(R::NonCanonicalEncoding)
        ));
    }
    assert!(matches!(
        View::decode(&raw(Q::ratio(1, 4, &mut work()).unwrap(), 11), &mut work()),
        Err(R::TrigonometricPole)
    ));
}
