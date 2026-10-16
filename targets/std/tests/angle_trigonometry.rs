//! Actual installed Host execution of typed Angle denotations; no device.
use conduit_core::{
    AngleTrigonometricFunction, ExactNumericWork, ExactRational, MathematicalEnclosureRefusal,
    MathematicalEnclosureRequirements, MathematicalValue, PreparedAngleTrigonometry,
    PreparedMathematicalEnclosure, Quantity, StructuredInfoValue, StructuredInfoValueShape,
    MATHEMATICAL_VALUE_VALIDATION_WORK,
};
use conduit_plot::parse_syntax_document;
#[path = "common/angle_host.rs"]
mod angle_host;
use angle_host::{prepare, run};
#[test]
fn installed_angle_values_keep_units_denotation_and_separate_numeric_projection() {
    for (name, function) in [
        ("math/sin", AngleTrigonometricFunction::Sine),
        ("math/cos", AngleTrigonometricFunction::Cosine),
        ("math/tan", AngleTrigonometricFunction::Tangent),
    ] {
        let source=format!("type AngleObservation = {{\n angle: Angle\n value: MathematicalValue\n}}\nplot trig (\n >> value: Angle\n result: AngleObservation >>\n) = ({{angle: ., value: {name}(.)}})\n");
        let (mut host, plan) = prepare(&source, "trig", "value");
        assert_eq!(
            plan.source_document_id,
            parse_syntax_document(&source).source_document_id()
        );
        for original in ["0turn", "180°", "1turn", "-1turn", "0.17turn"] {
            let input = Quantity::parse_plot_literal(original).unwrap();
            let outputs = run(&mut host, &plan, "value", vec![input.encode().to_vec()]);
            let value = StructuredInfoValue::from_canonical_bytes(&outputs[0]).unwrap();
            let StructuredInfoValueShape::Record(fields) = value.shape() else {
                panic!("record")
            };
            let payload = |name| {
                let field = fields.iter().find(|f| f.name() == name).unwrap();
                let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                    panic!("leaf")
                };
                bytes
            };
            let received = Quantity::decode(payload("angle")).unwrap();
            assert_eq!(received, input);
            let mut work = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
            let denotation = MathematicalValue::decode(payload("value"), &mut work).unwrap();
            let coordinate =
                PreparedAngleTrigonometry::from_checked(function, received, original, &mut work)
                    .unwrap();
            assert_eq!(coordinate.denotation(), &denotation);
            assert_eq!(coordinate.coordinate_projection().source(), input);
            let requirements = MathematicalEnclosureRequirements {
                maximum_width: ExactRational::ZERO,
                maximum_leaf_width: ExactRational::ZERO,
                maximum_constant_terms: 1,
                maximum_root_refinements: 1,
                maximum_scratch_payload_bytes: 100_000,
                maximum_retained_payload_bytes: 100_000,
            };
            match denotation.as_rational() {
                Some(q) => {
                    let numeric = PreparedMathematicalEnclosure::prepare(
                        &denotation,
                        requirements,
                        &mut work,
                    )
                    .unwrap();
                    assert_eq!(numeric.source(), &denotation);
                    assert_eq!((numeric.target().lower(), numeric.target().upper()), (q, q));
                }
                None => assert!(matches!(
                    PreparedMathematicalEnclosure::prepare(&denotation, requirements, &mut work),
                    Err(MathematicalEnclosureRefusal::UnsupportedTrigonometricEnclosure)
                )),
            }
        }
    }
}
