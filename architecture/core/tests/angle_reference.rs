use conduit_core::*;
use core::cmp::Ordering;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn convert<'a>(source: &'a str, target: &'a str) -> PreparedAngleReferenceConversion<'a> {
    PreparedAngleReferenceConversion::from_checked(
        Quantity::parse_plot_literal(source).unwrap(),
        Unit::resolve(target).unwrap(),
        source,
        target,
        &mut work(),
    )
    .unwrap()
}
#[test]
fn reference_law_keeps_pi_symbolic_and_original_angle_exact() {
    let half = convert("180°", "rad");
    assert_eq!(
        half.denotation(),
        &MathematicalValue::parse_literal("pi", &mut work()).unwrap()
    );
    assert_eq!(half.original(), "180°");
    assert_eq!(half.source(), Quantity::parse_plot_literal("180°").unwrap());
    assert_eq!(half.target().unit(), Unit::Radian);
    assert_eq!(half.reference_law(), TURN_RADIAN_REFERENCE_LAW);
    assert_eq!(
        half.turns().coordinate(),
        ExactRational::ratio(1, 2, &mut work()).unwrap()
    );
    assert_eq!(convert("1turn", "rad").denotation().as_rational(), None);
    assert_eq!(
        convert("0turn", "rad").denotation().as_rational(),
        Some(ExactRational::ZERO)
    );
    // The explicit bridge never changes the existing rational Unit conversion.
    assert!(Quantity::parse_plot_literal("1turn")
        .unwrap()
        .convert_to_unit(Unit::Radian)
        .is_err());
}
#[test]
fn target_prefix_is_exact_and_origins_are_not_guessed() {
    let source = convert("0.5turn", "mrad");
    let mut builder = MathematicalValueBuilder::new();
    let pi = builder.insert(MathematicalNode::Pi, &mut work()).unwrap();
    let thousand = builder
        .insert(
            MathematicalNode::Rational(ExactRational::from_i128(1000)),
            &mut work(),
        )
        .unwrap();
    let root = builder
        .insert(MathematicalNode::Multiply(thousand, pi), &mut work())
        .unwrap();
    assert_eq!(
        source.denotation(),
        &builder.finish(root, &mut work()).unwrap()
    );
    for (source, target) in [
        ("1m", "rad"),
        ("1turn", "m"),
        ("1rad", "rad"),
        ("1turn", "turn"),
    ] {
        assert!(PreparedAngleReferenceConversion::from_checked(
            Quantity::parse_plot_literal(source).unwrap(),
            Unit::resolve(target).unwrap(),
            source,
            target,
            &mut work(),
        )
        .is_err());
    }
    assert!(PreparedAngleReferenceConversion::from_checked(
        Quantity::new(1, Unit::Turn),
        Unit::Radian,
        "2turn",
        "rad",
        &mut work()
    )
    .is_err());
}
#[test]
fn symbolic_target_uses_existing_certified_enclosure_or_refuses() {
    let conversion = convert("1turn", "rad");
    let requirements = MathematicalEnclosureRequirements {
        maximum_width: ExactRational::ratio(1, 1_000_000, &mut work()).unwrap(),
        maximum_leaf_width: ExactRational::ratio(1, 100_000_000, &mut work()).unwrap(),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let enclosure =
        PreparedMathematicalEnclosure::prepare(conversion.denotation(), requirements, &mut work())
            .unwrap();
    assert_eq!(enclosure.source(), conversion.denotation());
    assert_eq!(
        enclosure
            .width()
            .checked_compare(requirements.maximum_width, &mut work())
            .unwrap(),
        Ordering::Less
    );
    assert!(enclosure.maximum_used_constant_terms() > 0);
    assert!(PreparedMathematicalEnclosure::prepare(
        conversion.denotation(),
        MathematicalEnclosureRequirements {
            maximum_constant_terms: 0,
            ..requirements
        },
        &mut work()
    )
    .is_err());
    assert!(PreparedNumericalRealization::prepare(
        conversion.denotation(),
        NumericalAlgorithm::ExactRational,
        ExactRational::ZERO,
        &mut work()
    )
    .is_err());
}
