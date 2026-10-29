//! Checked authored debounce policy; inert samples do not prove physical contacts.
use super::*;
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
use serde_json::{Value, json};

fn encode(ty: &StructuredInfoType, value: &Value) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(kind) => {
            let bytes = match kind.as_str() {
                "value/u64" => value.as_u64().unwrap().to_le_bytes().to_vec(),
                "value/bool" => vec![u8::from(value.as_bool().unwrap())],
                "value/bytes" => value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|byte| u8::try_from(byte.as_u64().unwrap()).unwrap())
                    .collect(),
                "value/empty" => vec![],
                kind => panic!("unexpected test leaf {kind}"),
            };
            StructuredInfoValue::leaf(ty.clone(), bytes).unwrap()
        }
        StructuredInfoTypeShape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        encode(field.value_type(), &value[field.name()]),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Variant { cases, .. } => {
            let (tag, payload) = value.as_object().unwrap().iter().next().unwrap();
            let case = cases.iter().find(|case| case.tag() == tag).unwrap();
            StructuredInfoValue::variant(ty.clone(), tag, encode(case.payload_type(), payload))
                .unwrap()
        }
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), encode(representation, value)).unwrap()
        }
        _ => panic!("unexpected test shape"),
    }
}

fn decode(value: &StructuredInfoValue) -> Value {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => {
            let StructuredInfoTypeShape::Leaf(kind) = value.value_type().shape() else {
                panic!("leaf")
            };
            match kind.as_str() {
                "value/u64" => json!(u64::from_le_bytes(bytes.try_into().unwrap())),
                "value/bool" => json!(bytes == [1]),
                "value/bytes" => json!(bytes),
                "value/empty" => Value::Null,
                kind => panic!("unexpected output leaf {kind}"),
            }
        }
        StructuredInfoValueShape::Record(fields) => Value::Object(
            fields
                .iter()
                .map(|field| (field.name().into(), decode(field.value())))
                .collect(),
        ),
        StructuredInfoValueShape::Variant { tag, payload } => {
            let mut object = serde_json::Map::new();
            object.insert(tag.into(), decode(payload));
            Value::Object(object)
        }
        _ => panic!("unexpected output shape"),
    }
}

struct Expression {
    input: StructuredInfoType,
    evaluators: Vec<PreparedPortableExpressionEvaluator>,
}
impl Expression {
    fn prepare(package: &[u8], entry: &str) -> Self {
        let prepared = PreparedProtocolEntry::prepare(package, entry)
            .unwrap_or_else(|error| panic!("{entry}: {error:?}"));
        let gears = &prepared.expanded().expanded.gears;
        let programs: Vec<_> = gears
            .iter()
            .map(|gear| {
                let ConfigurationValue::Text(hex) = &gear.configuration[0].value else {
                    panic!("program")
                };
                PortableExpressionProgram::from_canonical_hex(hex).unwrap()
            })
            .collect();
        Self {
            input: programs[0].input_type.clone(),
            evaluators: programs
                .iter()
                .map(|program| PreparedPortableExpressionEvaluator::new(program).unwrap())
                .collect(),
        }
    }
    fn evaluate(&mut self, input: &Value) -> Value {
        let value = encode(&self.input, input);
        let mut wire = match value.shape() {
            StructuredInfoValueShape::Leaf(bytes) => bytes.to_vec(),
            _ => value.canonical_bytes().unwrap(),
        };
        for evaluator in &mut self.evaluators {
            wire = evaluator.evaluate(&wire).unwrap().to_vec();
        }
        decode(&StructuredInfoValue::from_canonical_bytes(&wire).unwrap())
    }
}

fn package() -> Vec<u8> {
    let source = [
        include_str!("../../../../../../plots/device-protocols/two-button-types.conduit"),
        include_str!("../../../../../../plots/device-protocols/two-button.conduit"),
    ]
    .join("\n");
    serde_json::to_vec(&ProtocolSourcePackage::compile(source, &[]).unwrap()).unwrap()
}
fn initialized(package: &[u8]) -> Value {
    Expression::prepare(package, "two-button-initialize").evaluate(&json!(true))
}
fn sample(state: &Value, now: u64, a_high: bool, b_high: bool) -> Value {
    json!({"state":state,"sample":{
        "a":{"completed":{"high":a_high}},
        "b":{"completed":{"high":b_high}},
        "clock":{"completed":now},"sequence":state["sequence"]
    }})
}
fn no_edges(state: &Value) {
    for field in ["a_pressed", "b_pressed", "a_released", "b_released"] {
        assert_eq!(state[field], false, "{field}: {state}");
    }
}
#[test]
fn two_button_active_low_bounce_hold_release_and_simultaneous_edges() {
    let package = package();
    let mut transition = Expression::prepare(&package, "two-button-transition");
    let mut state = initialized(&package);
    for (now, a, b) in [
        (0, true, true),
        (1, false, true),
        (10, true, true),
        (11, false, true),
        (30, false, true),
    ] {
        state = transition.evaluate(&sample(&state, now, a, b));
        no_edges(&state);
    }
    state = transition.evaluate(&sample(&state, 31, false, true));
    assert_eq!(state["a_pressed"], true);
    assert_eq!(state["b_pressed"], false);
    state = transition.evaluate(&sample(&state, 100, false, true));
    no_edges(&state);
    state = transition.evaluate(&sample(&state, 101, true, true));
    no_edges(&state);
    state = transition.evaluate(&sample(&state, 121, true, true));
    assert_eq!(state["a_released"], true);
    state = transition.evaluate(&sample(&state, 122, false, false));
    no_edges(&state);
    state = transition.evaluate(&sample(&state, 142, false, false));
    assert_eq!(state["a_pressed"], true);
    assert_eq!(state["b_pressed"], true);
    state = transition.evaluate(&sample(&state, 143, false, false));
    no_edges(&state);
    state = transition.evaluate(&sample(&state, 144, true, true));
    state = transition.evaluate(&sample(&state, 164, true, true));
    assert_eq!(state["a_released"], true);
    assert_eq!(state["b_released"], true);
    assert_eq!(state["failure"], 0);
}
#[test]
fn two_button_stale_time_sequence_budget_and_provider_dispositions_are_terminal() {
    let package = package();
    let mut transition = Expression::prepare(&package, "two-button-transition");
    let initial = initialized(&package);
    let mut cases = vec![];
    let mut input = sample(&initial, 0, true, true);
    input["sample"]["sequence"] = json!(1);
    cases.push((input, 1));
    let mut stale = initial.clone();
    stale["sequence"] = json!(1);
    let mut input = sample(&stale, 0, true, true);
    input["sample"]["sequence"] = json!(0);
    cases.push((input, 1));
    let mut state = initial.clone();
    state["clock"] = json!(100);
    cases.push((sample(&state, 99, true, true), 1));
    state["clock"] = json!(0);
    state["since"] = json!(100);
    cases.push((sample(&state, 99, true, true), 1));
    state = initial.clone();
    state["sequence"] = json!(1048576);
    cases.push((sample(&state, 0, true, true), 3));
    for (field, tags, base) in [
        ("a", vec!["unsupported", "provider-lost", "refused"], 10),
        ("b", vec!["unsupported", "provider-lost", "refused"], 20),
        (
            "clock",
            vec![
                "unavailable",
                "provider-lost",
                "unsupported-deadline",
                "malformed",
                "timeout",
            ],
            30,
        ),
    ] {
        for (index, tag) in tags.iter().enumerate() {
            let mut input = sample(&initial, 0, true, true);
            input["sample"][field] = json!({*tag:null});
            cases.push((input, base + index));
        }
    }
    for (input, failure) in cases {
        let refused = transition.evaluate(&input);
        assert_eq!(refused["failure"], failure);
        no_edges(&refused);
        assert_eq!(
            transition.evaluate(&sample(&refused, 200, false, false)),
            refused
        );
    }
    let mut final_state = initial;
    final_state["sequence"] = json!(1048575);
    let final_state = transition.evaluate(&sample(&final_state, u64::MAX, true, true));
    assert_eq!(final_state["sequence"], 1048576);
    assert_eq!(final_state["failure"], 0);
    let refused = transition.evaluate(&sample(&final_state, u64::MAX, true, true));
    assert_eq!(refused["failure"], 3);
}

#[test]
fn two_button_prepared_evaluation_retains_finite_storage_without_allocation() {
    let package = package();
    let mut transition = Expression::prepare(&package, "two-button-transition");
    assert_eq!(transition.evaluators.len(), 1);
    let initial = initialized(&package);
    let inputs: Vec<_> = [
        sample(&initial, 0, true, true),
        sample(&initial, 20, false, false),
    ]
    .iter()
    .map(|value| encode(&transition.input, value).canonical_bytes().unwrap())
    .collect();
    let evaluator = &mut transition.evaluators[0];
    let capacity = evaluator.output_capacity();
    let allocations = crate::test_allocations::allocations(|| {
        for input in &inputs {
            assert!(evaluator.evaluate(input).is_ok());
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(evaluator.output_capacity(), capacity);
}
