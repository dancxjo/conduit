//! Authored polling topology and deterministic policy; no physical button proof.
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
        let expanded = &prepared.expanded().expanded;
        let mut ordered = Vec::new();
        while ordered.len() < expanded.gears.len() {
            let gear = expanded
                .gears
                .iter()
                .find(|gear| {
                    !ordered.contains(&gear.gear_id)
                        && expanded
                            .connections
                            .iter()
                            .filter(|connection| connection.sink_gear_id == gear.gear_id)
                            .all(|connection| ordered.contains(&connection.source_gear_id))
                })
                .expect("acyclic one-input expression pipeline");
            ordered.push(gear.gear_id.clone());
        }
        let programs: Vec<_> = ordered
            .iter()
            .map(|id| {
                let gear = expanded
                    .gears
                    .iter()
                    .find(|gear| &gear.gear_id == id)
                    .unwrap();
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
        include_str!(
            "../../../../../../plots/device-protocols/two-button-autonomous-imports.conduit"
        ),
        include_str!("../../../../../../plots/device-protocols/two-button-types.conduit"),
        include_str!("../../../../../../plots/device-protocols/two-button.conduit"),
        include_str!(
            "../../../../../../plots/device-protocols/two-button-autonomous-types.conduit"
        ),
        include_str!(
            "../../../../../../plots/device-protocols/two-button-autonomous-control.conduit"
        ),
        include_str!(
            "../../../../../../plots/device-protocols/two-button-autonomous-transition.conduit"
        ),
        include_str!("../../../../../../plots/device-protocols/two-button-autonomous.conduit"),
    ]
    .join("\n");
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    let package = ProtocolSourcePackage::compile(
        source,
        &[
            ProtocolSpecializationRequest::SeededUntilInstance {
                alias: "two-button-cell".into(),
                value: value("TwoButtonLoopFrame"),
            },
            ProtocolSpecializationRequest::FeedbackZipInstance {
                alias: "two-button-feedback".into(),
                paired_alias: "TwoButtonLoopPair".into(),
                left: value("TwoButtonLoopState"),
                right: value("TwoButtonLoopOutcome"),
            },
            ProtocolSpecializationRequest::MergeInstance {
                alias: "two-button-outcomes".into(),
                value: value("TwoButtonLoopOutcome"),
            },
        ],
    )
    .unwrap();
    serde_json::to_vec(&package).unwrap()
}
#[test]
fn two_button_autonomous_expands_distinct_read_contacts_clock_and_one_feedback_kernel() {
    let package = package();
    let entry = PreparedProtocolEntry::prepare(&package, "two-button-autonomous").unwrap();
    let gears = &entry.expanded().expanded.gears;
    let contacts: Vec<_> = gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == "machine/digital/access")
        .collect();
    assert_eq!(contacts.len(), 2);
    assert!(
        contacts
            .iter()
            .any(|gear| gear.gear_id.as_str().ends_with("/digital-a"))
    );
    assert!(
        contacts
            .iter()
            .any(|gear| gear.gear_id.as_str().ends_with("/digital-b"))
    );
    assert_eq!(
        gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "machine/clock/at")
            .count(),
        1
    );
    assert_eq!(entry.expanded().front.inputs().len(), 1);
    assert_eq!(entry.expanded().front.outputs().len(), 2);
    assert!(
        entry
            .input_schema(&conduit_core::port_id("start"))
            .is_some()
    );
}
fn initial(package: &[u8]) -> Value {
    let buttons = Expression::prepare(package, "two-button-initialize").evaluate(&json!(true));
    Expression::prepare(package, "two-button-loop-initialize").evaluate(&buttons)
}
fn event(transition: &mut Expression, state: Value, tag: &str, payload: Value) -> Value {
    transition.evaluate(&json!({"state":state,"outcome":{tag:payload}}))
}
#[test]
fn two_button_autonomous_poll_trace_uses_10ms_deadlines_and_existing_20ms_edge_policy() {
    let package = package();
    let mut action = Expression::prepare(&package, "two-button-loop-action");
    let mut transition = Expression::prepare(&package, "two-button-loop-transition");
    let mut debounce = Expression::prepare(&package, "two-button-transition");
    let mut state = initial(&package);
    for (sample_index, expected_a_edge) in [(0, false), (1, false), (2, true), (3, false)] {
        let deadline = (sample_index + 1) * 10;
        assert_eq!(
            action.evaluate(&state),
            json!({"clock":{"deadline":deadline}})
        );
        state = event(
            &mut transition,
            state,
            "clock",
            json!({"completed":deadline}),
        );
        assert_eq!(action.evaluate(&state), json!({"a":{"observe":null}}));
        state = event(
            &mut transition,
            state,
            "a",
            json!({"completed":{"high":false}}),
        );
        assert_eq!(action.evaluate(&state), json!({"b":{"observe":null}}));
        state = event(
            &mut transition,
            state,
            "b",
            json!({"completed":{"high":true}}),
        );
        let commit = action.evaluate(&state);
        let result = debounce.evaluate(&commit["commit"]);
        assert_eq!(result["a_pressed"], expected_a_edge);
        assert_eq!(result["b_pressed"], false);
        assert_eq!(result["sequence"], sample_index + 1);
        state = event(&mut transition, state, "commit", result);
        assert_eq!(state["phase"], 0);
    }
}
#[test]
fn two_button_autonomous_failure_skips_later_reads_and_lifetime_closes() {
    let package = package();
    let mut action = Expression::prepare(&package, "two-button-loop-action");
    let mut transition = Expression::prepare(&package, "two-button-loop-transition");
    let mut debounce = Expression::prepare(&package, "two-button-transition");
    for (phase, tag, outcome, failure) in [
        (0, "clock", json!({"provider-lost":null}), 31),
        (0, "clock", json!({"completed":9}), 1),
        (1, "a", json!({"unsupported":null}), 10),
        (2, "b", json!({"refused":null}), 22),
        (1, "b", json!({"completed":{"high":true}}), 1),
    ] {
        let mut state = initial(&package);
        state["phase"] = json!(phase);
        state = event(&mut transition, state, tag, outcome);
        assert_eq!(state["phase"], 3);
        let commit = action.evaluate(&state);
        let result = debounce.evaluate(&commit["commit"]);
        assert_eq!(result["failure"], failure);
        state = event(&mut transition, state, "commit", result);
        assert_eq!(action.evaluate(&state)["ended"]["reason"], failure);
        assert_eq!(state["phase"], 4);
    }
    let mut state = initial(&package);
    let mut final_buttons = state["buttons"].clone();
    final_buttons["sequence"] = json!(1048576);
    state["phase"] = json!(3);
    state = event(&mut transition, state, "commit", final_buttons);
    assert_eq!(action.evaluate(&state)["ended"]["reason"], 3);
    assert_eq!(state["phase"], 4);
}
