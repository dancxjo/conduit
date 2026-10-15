//! Checked Angle meaning through the ordinary portable expression entrance.
use conduit_core::{
    AngleTrigonometricFunction as F, AngleTrigonometryRefusal as Domain, ConfigurationValue,
    ExactNumericWork, MathematicalValue as V, MathematicalValueRefusal,
    PreparedAngleTrigonometry as Angle, Quantity, MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
    MATHEMATICAL_VALUE_VALIDATION_WORK,
};
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn source(input: &str, body: &str) -> String {
    format!("plot trig (\n >> value: {input}\n result: MathematicalValue >>\n) = ({body})\n")
}
fn program(body: &str) -> PortableExpressionProgram {
    compile(&source("Angle", body))
}
fn compile(text: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(text);
    assert_eq!(syntax.round_trip(), text);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "trig", &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn expected(function: F, original: &str) -> Vec<u8> {
    let quantity = Quantity::parse_plot_literal(original).unwrap();
    let value = Angle::from_checked(
        function,
        quantity,
        original,
        &mut ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK),
    )
    .unwrap();
    let mut output = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = value.denotation().encode(&mut output).unwrap();
    output.truncate(length);
    output
}
#[test]
fn exact_literal_and_dynamic_angle_calls_preserve_denotation_without_play_growth() {
    for (name, function) in [
        ("math/sin", F::Sine),
        ("math/cos", F::Cosine),
        ("math/tan", F::Tangent),
    ] {
        let program = program(&format!("{name}(.)"));
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let capacity = prepared.output_capacity();
        for original in ["0turn", "180°", "1turn", "-1turn", "0.125turn", "0.17turn"] {
            let input = Quantity::parse_plot_literal(original).unwrap().encode();
            let expected = expected(function, original);
            assert_eq!(program.evaluate(&input).unwrap(), expected);
            let (output, allocation) =
                allocation_probe::observe(|| prepared.evaluate(&input).unwrap());
            assert_eq!(output, expected);
            assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
            assert_eq!(prepared.output_capacity(), capacity);
        }
    }
    for literal in ["0.25turn", "90°"] {
        let program = program(&format!("math/sin({literal})"));
        let bytes = program
            .evaluate(&Quantity::parse_plot_literal("0turn").unwrap().encode())
            .unwrap();
        assert_eq!(
            V::decode(
                &bytes,
                &mut ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK)
            )
            .unwrap(),
            V::rational(conduit_core::ExactRational::ONE)
        );
    }
}
#[test]
fn scalar_and_wrong_dimension_calls_fail_angle_typechecking() {
    for (input, body) in [
        ("Scalar", "math/sin(.)"),
        ("Distance", "math/cos(.)"),
        ("Frequency", "math/tan(.)"),
        ("Angle", "math/sin(42)"),
        ("Angle", "math/sin(1m)"),
    ] {
        let checked = check_syntax_document(
            &parse_syntax_document(&source(input, body)),
            &StartupCatalog::new(),
        )
        .unwrap();
        match expand_canonical_plot_for_authoring(&checked, "trig", &ProfileCatalog::new()) {
            Err(error) => assert!(error.message.contains("Angle"), "{}", error.message),
            Ok(_) => panic!("wrong-dimension trig call admitted"),
        }
    }
}
#[test]
fn tangent_poles_and_unsupported_radian_projection_keep_named_domain_reasons() {
    let pole = PortableExpressionEvaluationRefusal::AngleTrigonometry(Domain::Denotation(
        MathematicalValueRefusal::TrigonometricPole,
    ));
    let program = program("math/tan(.)");
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for original in ["90°", "270°", "-0.25turn", "1.25turn"] {
        let input = Quantity::parse_plot_literal(original).unwrap().encode();
        assert_eq!(program.evaluate(&input), Err(pole.clone()));
        assert_eq!(prepared.evaluate(&input), Err(pole.clone()));
    }
    let input = Quantity::parse_plot_literal("1rad").unwrap().encode();
    assert!(matches!(
        prepared.evaluate(&input),
        Err(PortableExpressionEvaluationRefusal::AngleTrigonometry(
            Domain::Coordinate(_)
        ))
    ));
}

#[test]
fn source_carrier_preserves_original_angle_while_numeric_projection_is_separate() {
    use conduit_core::{
        ExactRational as Q, MathematicalEnclosureRequirements as Requirements,
        PreparedMathematicalEnclosure as Numeric, StructuredInfoValue as S,
        StructuredInfoValueShape as Shape,
    };
    let text="type AngleObservation = {\n angle: Angle\n value: MathematicalValue\n}\nplot trig (\n >> value: Angle\n result: AngleObservation >>\n) = ({angle: ., value: math/sin(.)})\n";
    let program = compile(text);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for original in ["90°", "0.25turn"] {
        let angle = Quantity::parse_plot_literal(original).unwrap();
        let reference = program.evaluate(&angle.encode()).unwrap();
        let (actual, allocation) =
            allocation_probe::observe(|| prepared.evaluate(&angle.encode()).unwrap());
        assert_eq!(actual, reference);
        assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
        let value = S::from_canonical_bytes(actual).unwrap();
        let Shape::Record(fields) = value.shape() else {
            panic!("record")
        };
        let payload = |name| {
            let field = fields.iter().find(|f| f.name() == name).unwrap();
            let Shape::Leaf(bytes) = field.value().shape() else {
                panic!("leaf")
            };
            bytes
        };
        assert_eq!(Quantity::decode(payload("angle")).unwrap(), angle);
        let mut work = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
        let denotation = V::decode(payload("value"), &mut work).unwrap();
        let coordinate = Angle::from_checked(F::Sine, angle, original, &mut work).unwrap();
        assert_eq!(coordinate.denotation(), &denotation);
        assert_eq!(coordinate.coordinate_projection().source(), angle);
        let requirements = Requirements {
            maximum_width: Q::ZERO,
            maximum_leaf_width: Q::ZERO,
            maximum_constant_terms: 1,
            maximum_root_refinements: 1,
            maximum_scratch_payload_bytes: 100_000,
            maximum_retained_payload_bytes: 100_000,
        };
        let numeric = Numeric::prepare(&denotation, requirements, &mut work).unwrap();
        assert_eq!(numeric.source(), &denotation);
        assert_eq!(
            (numeric.target().lower(), numeric.target().upper()),
            (Q::ONE, Q::ONE)
        );
    }
}

#[test]
fn declared_rational_turn_unit_retains_non_decimal_angle_denotation() {
    let text="unit seventhturn : Angle = {reference:turn,scale:1/7}\nplot trig (\n >> value: Angle\n result: MathematicalValue >>\n) = (math/sin(1seventhturn))\n";
    let program = compile(text);
    let input = Quantity::parse_plot_literal("0turn").unwrap().encode();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let reference = program.evaluate(&input).unwrap();
    let (actual, allocation) = allocation_probe::observe(|| prepared.evaluate(&input).unwrap());
    assert_eq!(actual, reference);
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let mut work = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
    let mut builder = conduit_core::MathematicalValueBuilder::new();
    let q = conduit_core::ExactRational::ratio(1, 7, &mut work).unwrap();
    let argument = builder
        .insert(conduit_core::MathematicalNode::Rational(q), &mut work)
        .unwrap();
    let sine = builder
        .insert(
            conduit_core::MathematicalNode::SineTurns(argument),
            &mut work,
        )
        .unwrap();
    let expected = builder.finish(sine, &mut work).unwrap();
    assert_eq!(V::decode(actual, &mut work).unwrap(), expected);
}

#[test]
fn known_constant_poles_refuse_during_preparation_before_any_play() {
    let value = program("math/tan(0.25turn)");
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new(&value),
        Err(PortableExpressionEvaluationRefusal::AngleTrigonometry(
            Domain::Denotation(MathematicalValueRefusal::TrigonometricPole)
        ))
    ));
    let value = program("math/sin(1rad)");
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new(&value),
        Err(PortableExpressionEvaluationRefusal::AngleTrigonometry(
            Domain::Coordinate(_)
        ))
    ));
}
