//! Deployed private envelope and complete typed deserializer boundaries.
use super::fixture;
use ciborium::value::Value;
use conduitos_creche_core::{
    data,
    wire::{MAX_BLOB_BYTES, Refusal},
};
fn original_value() -> Value {
    let (_, input) = fixture();
    let bytes = data::encode_interaction(&input).unwrap();
    ciborium::de::from_reader(&bytes[5..]).unwrap()
}
fn encoded(value: &Value) -> Vec<u8> {
    let mut bytes = b"CDCI\x01".to_vec();
    ciborium::ser::into_writer(value, &mut bytes).unwrap();
    bytes
}
fn field<'a>(value: &'a mut Value, name: &str) -> &'a mut Value {
    let Value::Map(fields) = value else {
        panic!("expected named map")
    };
    &mut fields
        .iter_mut()
        .find(|(key, _)| key == &Value::Text(name.into()))
        .unwrap()
        .1
}
#[test]
fn envelope_bounds_version_truncation_and_trailing_bytes_refuse() {
    let (_, input) = fixture();
    let bytes = data::encode_interaction(&input).unwrap();
    assert_eq!(data::decode_interaction(&[]).err(), Some(Refusal::Bounds));
    assert_eq!(
        data::decode_interaction(&vec![0; MAX_BLOB_BYTES as usize + 1]).err(),
        Some(Refusal::Bounds)
    );
    let mut wrong = bytes.clone();
    wrong[4] = 2;
    let mut trailing = bytes.clone();
    trailing.push(0);
    for malformed in [wrong, trailing, bytes[..bytes.len() - 1].to_vec()] {
        assert_eq!(
            data::decode_interaction(&malformed).err(),
            Some(Refusal::Malformed)
        );
    }
}
#[test]
fn duplicate_missing_and_unknown_request_fields_remain_malformed() {
    for mode in 0..3 {
        let mut value = original_value();
        let Value::Map(fields) = &mut value else {
            panic!()
        };
        let index = fields
            .iter()
            .position(|(key, _)| key == &Value::Text("revision".into()))
            .unwrap();
        match mode {
            0 => {
                fields.push(fields[index].clone());
            }
            1 => {
                fields.remove(index);
            }
            _ => fields.push((Value::Text("unknown-private-field".into()), Value::Null)),
        }
        assert_eq!(
            data::decode_interaction(&encoded(&value)).err(),
            Some(Refusal::Malformed)
        );
    }
}
#[test]
fn ignored_nested_values_cannot_bypass_the_existing_recursion64_limit() {
    for (arrays, accepted) in [(60, true), (61, false)] {
        let mut value = original_value();
        let plan = field(field(field(&mut value, "show"), "planned_mask"), "plan");
        let Value::Map(fields) = plan else { panic!() };
        let mut nested = Value::Null;
        for _ in 0..arrays {
            nested = Value::Array(vec![nested]);
        }
        // Request, Show, binary Mask and Plan already consume four containers.
        // The unknown Plan field is ignored normally, but still bounded.
        fields.push((Value::Text("unknown-depth-fixture".into()), nested));
        let decoded = data::decode_interaction(&encoded(&value));
        assert_eq!(decoded.is_ok(), accepted, "nested arrays={arrays}");
        if let Ok(request) = decoded {
            assert!(conduit_core::verify_plan(&request.show.planned_mask.plan));
        }
    }
}

#[test]
fn exact_capacity_typed_request_survives_but_larger_transfers_refuse() {
    let (_, input) = fixture();
    let bytes = data::encode_interaction(&input).unwrap();
    // CBOR tags are transparent to the typed decoder. Pad a genuine complete
    // request to the exact transport cap without substituting invalid bytes.
    let mut exact = b"CDCI\x01".to_vec();
    exact.resize(MAX_BLOB_BYTES as usize - bytes.len() + 5, 0xc0);
    exact.extend_from_slice(&bytes[5..]);
    assert_eq!(exact.len(), MAX_BLOB_BYTES as usize);
    assert_eq!(data::decode_interaction(&exact).unwrap().show, input.show);
    exact.push(0);
    assert_eq!(
        data::decode_interaction(&exact).err(),
        Some(Refusal::Bounds)
    );

    let mut huge = input;
    huge.show.show.front_subject = "x".repeat(MAX_BLOB_BYTES as usize);
    assert_eq!(
        data::encode_interaction(&huge).err(),
        Some(Refusal::Capacity)
    );
}

#[test]
fn typed_activation_plans_cannot_bypass_the_private_recursion_limit() {
    use conduit_core::*;
    let (_, mut input) = fixture();
    let mut nested = input.show.planned_mask.plan.clone();
    nested.fragments.clear();
    nested.activations.clear();
    nested.activation_preparations.clear();
    let front = PlannedActivationFront {
        front_port_id: "fixture/front".into(),
        value_kind: "value/bool".into(),
        abnormal_kind: None,
    };
    for _ in 0..20 {
        let mut parent = nested.clone();
        parent.activations.clear();
        parent
            .activations
            .push(PlannedActivationEntry::Unary(PlannedActivation {
                activation_id: "fixture/nested".into(),
                owner_placement_id: "placement/fixture".into(),
                selected_plan_id: nested.plan_id.clone(),
                selected_plan: Box::new(nested),
                input: front.clone(),
                output: front.clone(),
                limits: PlannedActivationLimits {
                    maximum_active: 1,
                    maximum_queue_items: 1,
                    maximum_queue_bytes: 1,
                    maximum_items: 1,
                },
                terminal_policy: PlannedActivationTerminalPolicy::DrainThenPropagateExact,
                cancellation_policy:
                    PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
                effect_multiplicity: PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
                per_activation_sign_budget: SignStorageBudget {
                    item_capacity: 1,
                    byte_capacity: 1,
                },
            }));
        nested = parent;
    }
    input.show.planned_mask.plan = nested;
    let bytes = data::encode_interaction(&input).unwrap();
    // This is structurally typed input, not an admitted executable Plan. A high
    // reference limit parses it, while the real private limit must refuse it.
    let _: data::InteractionRequest =
        ciborium::de::from_reader_with_recursion_limit(&bytes[5..], 256).unwrap();
    assert_eq!(
        data::decode_interaction(&bytes).err(),
        Some(conduitos_creche_core::wire::Refusal::Malformed)
    );
}
