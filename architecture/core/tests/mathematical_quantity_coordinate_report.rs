//! Unit-coordinate exactness and numeric precision have independent reports.
use conduit_core::{projection::*, *};
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
use allocation_probe::observe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn frame(symbolic: bool, unit: Unit, role: QuantityRole) -> Vec<u8> {
    let v = if symbolic {
        MathematicalValue::parse_literal("pi", &mut work()).unwrap()
    } else {
        MathematicalValue::rational(ExactRational::ONE)
    };
    frame_value(&v, unit, role)
}
fn frame_value(v: &MathematicalValue, unit: Unit, role: QuantityRole) -> Vec<u8> {
    let mut numeric = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = v.encode(&mut numeric).unwrap();
    let view = MathematicalValueView::decode(&numeric[..n], &mut work()).unwrap();
    let mut out = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(&view, unit, role, &mut out, &mut work()).unwrap();
    out.truncate(n);
    out
}
fn input<'a, D: ProjectionDomain>(
    source: &'a D::Source,
    target: &'a D::Target,
    facts: &'a [ProjectionFact<'a, D>],
    route: &'a str,
) -> ProjectionInput<'a, D> {
    ProjectionInput {
        basis: ProjectionBasis {
            report: text(route),
            source: text("quantity/source@1"),
            target: Some(text("quantity/target@1")),
            projector: text(route),
            requested_route: text(route),
            boundary: None,
        },
        source,
        target: Some(target),
        facts,
        native: &[],
        scores: &[],
        admitted: &[],
        attempts: &[],
        selected_attempt: None,
        mechanism: ProjectionMechanism::Completed,
        diagnostics: &[],
    }
}
#[test]
fn exact_unit_change_and_lossy_numeric_realization_have_separate_checked_reports() {
    let source = frame(true, Unit::Meter, QuantityRole::Linear);
    let mut converter = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let req = MathematicalEnclosureRequirements {
        maximum_width: q(1, 100_000_000_000_000),
        maximum_leaf_width: q(1, 100_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let mut evaluator = PreparedMathematicalQuantityEnclosure::new(req, 200_000_000).unwrap();
    let unit_domain = MathematicalQuantityConversionDomain::new();
    let unit_policy = MathematicalQuantityConversionPolicy {
        target_unit: Unit::Centimeter,
    };
    let numeric_domain = MathematicalQuantityEnclosureDomain::new();
    let numeric_policy = MathematicalQuantityEnclosurePolicy { requirements: req };
    let (_, allocations) = observe(|| {
        for _ in 0..4 {
            let conversion = converter
                .execute(&source, Unit::Centimeter, &mut output, &mut work())
                .unwrap();
            let bytes = &output[..conversion.encoded_bytes()];
            let target = MathematicalQuantityView::decode(bytes, &mut work()).unwrap();
            let unit_facts = unit_domain.facts();
            let unit_report = ProjectionReport::new(
                &unit_domain,
                &unit_policy,
                input(&conversion, &target, &unit_facts, "quantity/unit-route@1"),
            )
            .unwrap();
            assert!(unit_report.require_exact().is_ok());
            assert_eq!(conversion.source().encoded(), source);
            assert_eq!(
                conversion.target_representation_digest(),
                target.representation_digest()
            );
            let numeric = evaluator.execute(bytes, &mut work()).unwrap();
            let numeric_target = numeric.target();
            let numeric_facts = numeric.facts();
            let numeric_report = ProjectionReport::new(
                &numeric_domain,
                &numeric_policy,
                input(
                    &numeric,
                    &numeric_target,
                    &numeric_facts,
                    "quantity/numeric-route@1",
                ),
            )
            .unwrap();
            assert!(numeric_report.require_exact().is_err());
            assert!(numeric_report
                .require_policy(text("numeric/quantity-coordinate-interval-policy@1"))
                .is_ok());
            assert_eq!(numeric.source().encoded(), target.encoded());
            assert_eq!(numeric_target.unit, Unit::Centimeter);
            assert_eq!(numeric_target.role, QuantityRole::Linear);
        }
    });
    assert_eq!(allocations, 0);
}
#[test]
fn coordinate_report_rejects_other_outputs_roles_units_facts_and_policy() {
    let source = frame(true, Unit::Fahrenheit, QuantityRole::Point);
    // A valid affine target with its Kelvin coordinate shifted by one.
    let mut builder = MathematicalValueBuilder::new();
    let pi = builder.insert(MathematicalNode::Pi, &mut work()).unwrap();
    let factor = builder
        .insert(MathematicalNode::Rational(q(5, 9)), &mut work())
        .unwrap();
    let scaled = builder
        .insert(MathematicalNode::Multiply(pi, factor), &mut work())
        .unwrap();
    let shifted = builder
        .insert(MathematicalNode::Rational(q(46147, 180)), &mut work())
        .unwrap();
    let root = builder
        .insert(MathematicalNode::Add(scaled, shifted), &mut work())
        .unwrap();
    let wrong_value = frame_value(
        &builder.finish(root, &mut work()).unwrap(),
        Unit::Kelvin,
        QuantityRole::Point,
    );
    let wrong_role = frame(true, Unit::Kelvin, QuantityRole::Delta);
    let wrong_unit = frame(true, Unit::Fahrenheit, QuantityRole::Point);
    let bad: [MathematicalQuantityView<'_>; 3] = [&wrong_value, &wrong_role, &wrong_unit]
        .map(|b| MathematicalQuantityView::decode(b, &mut work()).unwrap());
    let mut converter = PreparedMathematicalQuantityConversion::new(200000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let receipt = converter
        .execute(&source, Unit::Kelvin, &mut output, &mut work())
        .unwrap();
    let target =
        MathematicalQuantityView::decode(&output[..receipt.encoded_bytes()], &mut work()).unwrap();
    assert_eq!(wrong_value.len(), receipt.encoded_bytes());
    let domain = MathematicalQuantityConversionDomain::new();
    let facts = domain.facts();
    let policy = MathematicalQuantityConversionPolicy {
        target_unit: Unit::Kelvin,
    };
    let wrong_policy = MathematicalQuantityConversionPolicy {
        target_unit: Unit::Fahrenheit,
    };
    let false_facts = [
        ProjectionFact::Preserved {
            obligation: text("physical-denotation"),
        },
        ProjectionFact::Preserved {
            obligation: text("quantity-family-role"),
        },
        ProjectionFact::Preserved {
            obligation: text("unit-coordinates"),
        },
    ];
    let (_, allocations) = observe(|| {
        assert!(ProjectionReport::new(
            &domain,
            &policy,
            input(&receipt, &target, &facts, "quantity/unit-route@1")
        )
        .unwrap()
        .require_exact()
        .is_ok());
        for target in &bad {
            assert!(ProjectionReport::new(
                &domain,
                &policy,
                input(&receipt, target, &facts, "quantity/unit-route@1")
            )
            .is_err());
        }
        assert!(ProjectionReport::new(
            &domain,
            &policy,
            input(&receipt, &target, &facts[..2], "quantity/unit-route@1")
        )
        .is_err());
        assert!(ProjectionReport::new(
            &domain,
            &policy,
            input(&receipt, &target, &false_facts, "quantity/unit-route@1")
        )
        .is_err());
        assert!(!wrong_policy.permits(&receipt, &target, &facts));
    });
    assert_eq!(allocations, 0);
}
