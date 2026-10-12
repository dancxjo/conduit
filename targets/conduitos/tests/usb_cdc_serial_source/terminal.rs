//! Native failures must retain their class-neutral semantic terminal truth.
use super::*;
fn eval_both(
    program: &PortableExpressionProgram,
    value: &StructuredInfoValue,
) -> StructuredInfoValue {
    let bytes = value.canonical_bytes().unwrap();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(program).unwrap();
    assert_eq!(
        prepared.evaluate(&bytes).unwrap(),
        program.evaluate(&bytes).unwrap()
    );
    eval(program, value)
}
#[test]
fn native_transfer_failure_tags_survive_serial_projection_and_fence_next_effect() {
    let source = prepared();
    let accept = program(&source, "cdc-serial-accept");
    let advance = program(&source, "cdc-serial-advance");
    let emission = program(&source, "cdc-serial-emission");
    let state_type = fields(&accept.input_type)[0].value_type();
    let event_type = fields(&accept.input_type)[1].value_type();
    for (phase, event_tag) in [(6, "write"), (7, "read")] {
        let native_type = payload(event_type, event_tag);
        let StructuredInfoTypeShape::Variant { cases, .. } = native_type.shape() else {
            panic!("variant")
        };
        for fault in cases.iter().filter(|c| c.tag() != "completed") {
            let native = StructuredInfoValue::variant(
                native_type.clone(),
                fault.tag(),
                leaf(fault.payload_type(), &[]),
            )
            .unwrap();
            let event =
                StructuredInfoValue::variant(event_type.clone(), event_tag, native).unwrap();
            let previous = state(state_type, phase, 0);
            let input = pair(&accept.input_type, &previous, &event);
            let transition = eval_both(&accept, &input);
            assert_eq!(scalar(member(&transition, "accepted")), [0]);
            let emitted = eval_both(&emission, &transition).canonical_bytes().unwrap();
            let value = validate_canonical_structured_value(&emitted).unwrap();
            let result = value.variant_payload("emit").unwrap().unwrap();
            let expected = match fault.tag() {
                "stalled" => "stalled",
                "provider-lost" => "provider-lost",
                "timeout" => "timeout",
                _ => "unsupported",
            };
            assert_eq!(result.variant_tag().unwrap(), expected);
            let next = eval_both(&advance, &transition);
            assert_eq!(scalar(member(&next, "phase")), 9u64.to_le_bytes());
            assert_eq!(scalar(member(&next, "operations")), 0u64.to_le_bytes());
            // Phase 9 drains queued commands/closure through the ready arm;
            // it must not select a control, read or write arm.
            let route = program(&source, "cdc-serial-route");
            let bytes = eval_both(&route, &next).canonical_bytes().unwrap();
            assert_eq!(
                validate_canonical_structured_value(&bytes)
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "ready"
            );
        }
    }
}
