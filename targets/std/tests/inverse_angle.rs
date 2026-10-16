//! Installed ordinary Source/Plan/Play evidence; no physical timing claim.
use conduit_core::*;
use conduit_plot::rust_binding::primitive_into_structured;
use conduit_plot::{parse_syntax_document, PortableExpressionProgram};
#[path = "common/angle_host.rs"]
mod angle_host;
use angle_host::{prepare, run, run_observed};
const SOURCE: &str = include_str!("fixtures/inverse-angle.conduit");
fn work() -> ExactNumericWork {
    ExactNumericWork::new(1_000_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn pair(program: &PortableExpressionProgram, y: Quantity, x: Quantity) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "y",
                primitive_into_structured(fields[0].value_type().clone(), &y).unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "x",
                primitive_into_structured(fields[1].value_type().clone(), &x).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn leaf<'a>(record: &'a StructuredInfoValue, name: &str) -> &'a [u8] {
    let StructuredInfoValueShape::Record(fields) = record.shape() else {
        panic!()
    };
    let value = fields.iter().find(|f| f.name() == name).unwrap().value();
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!()
    };
    bytes
}
#[test]
fn installed_atan2_retains_source_units_exact_angle_and_correlated_requests() {
    let (mut host, plan) = prepare(SOURCE, "inverse", "value");
    assert_eq!(
        plan.source_document_id,
        parse_syntax_document(SOURCE).source_document_id()
    );
    let ConfigurationValue::Text(encoded) = &plan.fragments[0].placements[0].configuration[0].value
    else {
        panic!()
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut plays = 0;
    for (y, x) in [
        ("3m", "400cm"),
        ("-3m", "400cm"),
        ("3m", "-400cm"),
        ("-3m", "-400cm"),
        ("0m", "-4m"),
    ] {
        let y_value = Quantity::parse_plot_literal(y).unwrap();
        let x_value = Quantity::parse_plot_literal(x).unwrap();
        let input = pair(&program, y_value, x_value);
        let outputs = run_observed(&mut host, &plan, "value", vec![input.clone()], |kernel| {
            plays += 1;
            let mut requests = 0;
            for event in &kernel.kernel_sign {
                if event.kind != conduit_kernel::KernelEventKind::HostCallRequested {
                    continue;
                }
                requests += 1;
                let request = event.request.unwrap();
                let identity = kernel.identity.request(event.node, request).unwrap();
                assert!(plan.fragments[0].placements.iter().any(|p| p
                    .host_calls
                    .iter()
                    .any(|c| c.contract_id == identity.contract_id)));
                assert!(kernel.kernel_sign.iter().any(|completed| completed.kind
                    == conduit_kernel::KernelEventKind::HostCallCompleted
                    && completed.node == event.node
                    && completed.request == Some(request)
                    && completed.sequence > event.sequence));
            }
            assert!(requests > 0);
        });
        let output = StructuredInfoValue::from_canonical_bytes(&outputs[0]).unwrap();
        let StructuredInfoValueShape::Record(fields) = output.shape() else {
            panic!()
        };
        let original = fields
            .iter()
            .find(|f| f.name() == "original")
            .unwrap()
            .value();
        assert_eq!(original.canonical_bytes().unwrap(), input);
        let received = MathematicalAngle::decode(leaf(&output, "angle"), &mut work()).unwrap();
        assert_eq!(received.unit(), Unit::Turn);
        let exact = QuantityAtan2Denotation::from_checked(
            Atan2QuantityInputs {
                y: y_value,
                original_y: y,
                x: x_value,
                original_x: x,
                common_unit: Unit::resolve("m").unwrap(),
                common_unit_source: "m",
            },
            Atan2Branch::PrincipalSignedHalfTurn,
            &mut work(),
        )
        .unwrap();
        assert_eq!(received.coordinate(), exact.coordinate());
        let numeric = PreparedMathematicalEnclosure::prepare(
            received.coordinate(),
            MathematicalEnclosureRequirements {
                maximum_width: q(1, 1_000_000_000),
                maximum_leaf_width: q(1, 1_000_000_000),
                maximum_constant_terms: 32,
                maximum_root_refinements: 64,
                maximum_scratch_payload_bytes: 131072,
                maximum_retained_payload_bytes: 131072,
            },
            &mut work(),
        )
        .unwrap();
        assert_eq!(numeric.source(), received.coordinate());
        assert_ne!(
            numeric
                .width()
                .checked_compare(q(1, 1_000_000_000), &mut work())
                .unwrap(),
            core::cmp::Ordering::Greater
        );
    }
    assert_eq!(plays, 5);
}

#[test]
fn installed_unary_inverse_calls_keep_exact_input_and_bounded_projection() {
    for (name, node) in [
        ("math/atan", MathematicalNode::ArctangentTurns as fn(_) -> _),
        ("math/asin", MathematicalNode::ArcsineTurns),
        ("math/acos", MathematicalNode::ArccosineTurns),
    ] {
        let source=format!("type Observation = {{\n original: MathematicalValue\n angle: MathematicalAngle\n}}\n\
            plot inverse (\n >> value: MathematicalValue\n result: Observation >>\n) = ({{original: ., angle: {name}(.)}})\n");
        let (mut host, plan) = prepare(&source, "inverse", "value");
        assert_eq!(
            plan.source_document_id,
            parse_syntax_document(&source).source_document_id()
        );
        for numerator in [-3, 0, 3] {
            let input = MathematicalValue::rational(q(numerator, 5));
            let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
            let n = input.encode(&mut bytes).unwrap();
            bytes.truncate(n);
            let outputs = run(&mut host, &plan, "value", vec![bytes.clone()]);
            let value = StructuredInfoValue::from_canonical_bytes(&outputs[0]).unwrap();
            assert_eq!(leaf(&value, "original"), bytes);
            let angle = MathematicalAngle::decode(leaf(&value, "angle"), &mut work()).unwrap();
            let mut builder = MathematicalValueBuilder::new();
            let arg = builder
                .insert(MathematicalNode::Rational(q(numerator, 5)), &mut work())
                .unwrap();
            let root = builder.insert(node(arg), &mut work()).unwrap();
            assert_eq!(
                angle.coordinate(),
                &builder.finish(root, &mut work()).unwrap()
            );
            let numeric = PreparedMathematicalEnclosure::prepare(
                angle.coordinate(),
                MathematicalEnclosureRequirements {
                    maximum_width: q(1, 100_000_000),
                    maximum_leaf_width: q(1, 1_000_000_000),
                    maximum_constant_terms: 32,
                    maximum_root_refinements: 64,
                    maximum_scratch_payload_bytes: 131072,
                    maximum_retained_payload_bytes: 131072,
                },
                &mut work(),
            )
            .unwrap();
            assert_eq!(numeric.source(), angle.coordinate());
            assert_ne!(
                numeric
                    .width()
                    .checked_compare(q(1, 100_000_000), &mut work())
                    .unwrap(),
                core::cmp::Ordering::Greater
            );
        }
    }
}
