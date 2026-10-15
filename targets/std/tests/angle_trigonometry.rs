//! Actual installed Host execution of typed Angle denotations; no device.
use conduit_core::{
    AngleTrigonometricFunction, BaseImplementationId, ConfigurationValue, ConnectionTrack,
    ExactNumericWork, ExactRational, MathematicalEnclosureRefusal,
    MathematicalEnclosureRequirements, MathematicalValue, PortDirection, PreparedAngleTrigonometry,
    PreparedMathematicalEnclosure, Quantity, StructuredInfoValue, StructuredInfoValueShape,
    MATHEMATICAL_VALUE_VALIDATION_WORK,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("typed trigonometry has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare(source: &str, entry: &str, input_port: &str) -> (StdHost, conduit_core::Plan) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        if !advertisement
            .capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, input_port),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 65536,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 65536,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    (host, plan)
}
fn run(
    host: &mut StdHost,
    plan: &conduit_core::Plan,
    port: &str,
    inputs: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let count = inputs.len();
    let inputs = inputs
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: port.into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .collect::<Vec<_>>();
    let mut output = Outputs(Vec::with_capacity(count));
    for input in &inputs {
        // Each authored Item advances one finite ordinary Play. Caller batching
        // and fresh-host partitioning do not introduce a second runtime loop.
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                std::slice::from_ref(input),
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
    assert_eq!(output.0.len(), count);
    output.0
}

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
