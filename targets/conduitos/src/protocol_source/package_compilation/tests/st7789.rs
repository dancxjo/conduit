//! Exercise authored ST7789 Source through the public package/entry path.
use super::*;

fn source() -> String {
    [
        include_str!("../../../../../../plots/device-protocols/st7789-types.conduit"),
        include_str!("../../../../../../plots/device-protocols/st7789-control.conduit"),
        include_str!("../../../../../../plots/device-protocols/st7789-transition.conduit"),
    ]
    .join("\n")
}

#[test]
fn st7789_source_entries_compile_through_public_package() {
    let package = ProtocolSourcePackage::compile(source(), &[]).unwrap();
    let encoded = serde_json::to_vec(&package).unwrap();
    for entry in [
        "st7789-command-bank",
        "st7789-bank-begin",
        "st7789-step-high",
        "st7789-step-length",
        "st7789-step-delay",
        "st7789-protocol-initialize",
        "st7789-protocol-action",
        "st7789-step-begin",
        "st7789-step-safe-off",
        "st7789-step-backlight",
        "st7789-step-command-data",
        "st7789-step-spi",
        "st7789-step-clock",
        "st7789-step-chunk",
        "st7789-step-finish",
        "st7789-protocol-transition",
        "st7789-protocol-frame",
        "st7789-protocol-frame-value",
    ] {
        PreparedProtocolEntry::prepare(&encoded, entry)
            .unwrap_or_else(|error| panic!("{entry}: {error:?}"));
    }
}

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
        let mut wire = encode(&self.input, input).canonical_bytes().unwrap();
        for evaluator in &mut self.evaluators {
            wire = evaluator.evaluate(&wire).unwrap().to_vec();
        }
        decode(&StructuredInfoValue::from_canonical_bytes(&wire).unwrap())
    }
}

fn bank() -> Vec<u8> {
    conduit_data::bytes_literal::decode_hex("01113a5536c02113292a000000ef2b0050013f2c").unwrap()
}
fn package() -> Vec<u8> {
    serde_json::to_vec(&ProtocolSourcePackage::compile(source(), &[]).unwrap()).unwrap()
}

#[test]
fn st7789_complete_frame_requires_ordered_gpio_spi_clock_and_exact_chunks() {
    let package = package();
    let mut initialize = Expression::prepare(&package, "st7789-protocol-initialize");
    let mut action = Expression::prepare(&package, "st7789-protocol-action");
    let mut transition = Expression::prepare(&package, "st7789-protocol-transition");
    let mut state = initialize.evaluate(&json!({"bank": bank()}));
    let mut exchanges = vec![];
    let mut deadlines = vec![];
    let mut illuminated = false;
    let mut command_data_levels = vec![];
    let mut now = 100_u64;
    for _ in 0..600 {
        let command = action.evaluate(&state);
        let (tag, payload) = command.as_object().unwrap().iter().next().unwrap();
        if tag == "completed" {
            assert_eq!(payload["value"], 225);
            assert!(illuminated);
            break;
        }
        let outcome = match tag.as_str() {
            "backlight" | "command-data" => {
                let high = payload["request"]["drive"]["high"].as_bool().unwrap();
                if tag == "command-data" {
                    command_data_levels.push(high);
                }
                if tag == "backlight" && high {
                    assert_eq!(state["chunks"], 225);
                    illuminated = true;
                }
                json!({tag: {"completed": {"high": high}}})
            }
            "spi" => {
                let bytes = payload["request"]["output"].as_array().unwrap().clone();
                exchanges.push(bytes.clone());
                json!({"spi": {"completed": {"input": bytes}}})
            }
            "clock" => {
                let deadline = payload["request"]["deadline"].as_u64().unwrap();
                if deadline != 0 {
                    deadlines.push(deadline - now);
                    now = deadline;
                }
                json!({"clock": {"completed": now}})
            }
            "awaiting-chunk" => {
                json!({"chunk": {"index": payload["index"], "wire": vec![payload["index"].as_u64().unwrap() as u8; 512]}})
            }
            unexpected => panic!("unexpected action {unexpected}: {command}"),
        };
        state = transition.evaluate(
            &json!({"state": state, "event": {"ordinal": payload["ordinal"], "outcome": outcome}}),
        );
    }
    assert_eq!(state["phase"], 8);
    assert_eq!(deadlines, [150, 500, 10, 10, 500]);
    assert_eq!(exchanges.len(), 14 + 225);
    assert_eq!(
        command_data_levels,
        [
            false, false, false, true, false, true, false, false, false, false, true, false, true,
            false, true
        ]
    );
    let prefix: &[&[u8]] = &[
        &[1],
        &[17],
        &[58],
        &[85],
        &[54],
        &[192],
        &[33],
        &[19],
        &[41],
        &[42],
        &[0, 0, 0, 239],
        &[43],
        &[0, 80, 1, 63],
        &[44],
    ];
    for (actual, expected) in exchanges.iter().zip(prefix) {
        assert_eq!(
            actual,
            &expected.iter().map(|byte| json!(*byte)).collect::<Vec<_>>()
        );
    }
    for (index, bytes) in exchanges[14..].iter().enumerate() {
        assert!(bytes.iter().all(|byte| byte == &json!(index as u8)));
    }
    assert_eq!(
        exchanges[10],
        json!([0, 0, 0, 239]).as_array().unwrap().clone()
    );
    assert_eq!(
        exchanges[12],
        json!([0, 80, 1, 63]).as_array().unwrap().clone()
    );
    assert!(exchanges[14..].iter().all(|bytes| bytes.len() == 512));
}

#[test]
fn st7789_bad_completions_require_safe_off_before_terminal_refusal() {
    let package = package();
    let mut initialize = Expression::prepare(&package, "st7789-protocol-initialize");
    let mut transition = Expression::prepare(&package, "st7789-protocol-transition");
    let mut action = Expression::prepare(&package, "st7789-protocol-action");
    let initial = initialize.evaluate(&json!({"bank": bank()}));
    for (phase, outcome, ordinal, expected_failure) in [
        (
            0,
            json!({"backlight": {"completed": {"high": true}}}),
            0,
            13,
        ),
        (1, json!({"command-data": {"provider-lost": null}}), 0, 11),
        (2, json!({"spi": {"completed": {"input": []}}}), 0, 1),
        (4, json!({"clock": {"completed": 0}}), 0, 1),
        (
            5,
            json!({"chunk": {"index": 1, "wire": vec![0; 512]}}),
            0,
            1,
        ),
        (
            5,
            json!({"chunk": {"index": 0, "wire": vec![0; 511]}}),
            0,
            1,
        ),
        (6, json!({"spi": {"timed-out": null}}), 0, 30),
        (8, json!({"closed": null}), 0, 3),
        (
            0,
            json!({"backlight": {"completed": {"high": false}}}),
            1,
            2,
        ),
    ] {
        let mut state = initial.clone();
        state["phase"] = json!(phase);
        state["deadline"] = json!(500);
        let state = transition
            .evaluate(&json!({"state": state, "event": {"ordinal": ordinal, "outcome": outcome}}));
        assert_eq!(state["phase"], 9);
        assert_eq!(state["failure"], expected_failure);
        let off = action.evaluate(&state);
        assert_eq!(off["backlight"]["request"]["drive"]["high"], false);
        let state = transition.evaluate(&json!({"state": state, "event": {"ordinal": off["backlight"]["ordinal"], "outcome": {"backlight": {"completed": {"high": false}}}}}));
        assert_eq!(state["phase"], 10);
        assert!(action.evaluate(&state).get("refused").is_some());
    }
    let mut changed = bank();
    changed[5] = 0;
    assert_eq!(initialize.evaluate(&json!({"bank": changed}))["phase"], 9);
}

#[test]
fn st7789_provider_dispositions_and_exhaustion_keep_safe_off_explicit() {
    let package = package();
    let mut initialize = Expression::prepare(&package, "st7789-protocol-initialize");
    let mut transition = Expression::prepare(&package, "st7789-protocol-transition");
    let initial = initialize.evaluate(&json!({"bank": bank()}));
    for (phase, family, disposition, expected) in [
        (2, "spi", "timed-out", 30),
        (2, "spi", "provider-lost", 31),
        (2, "spi", "unsupported", 32),
        (2, "spi", "stale-attachment", 33),
        (2, "spi", "refused", 34),
        (0, "backlight", "unsupported", 10),
        (0, "backlight", "provider-lost", 11),
        (0, "backlight", "refused", 12),
        (3, "clock", "unavailable", 20),
        (3, "clock", "provider-lost", 21),
        (3, "clock", "unsupported-deadline", 22),
        (3, "clock", "malformed", 23),
        (3, "clock", "timeout", 24),
    ] {
        let mut state = initial.clone();
        state["phase"] = json!(phase);
        let state = transition.evaluate(&json!({"state": state, "event": {"ordinal": 0, "outcome": {family: {disposition: null}}}}));
        assert_eq!(state["phase"], 9);
        assert_eq!(state["failure"], expected);
    }
    let mut exhausted = initial.clone();
    exhausted["ordinal"] = json!(u64::MAX);
    let off = transition.evaluate(
        &json!({"state": exhausted, "event": {"ordinal": u64::MAX, "outcome": {"closed": null}}}),
    );
    assert_eq!(off["phase"], 9);
    let terminal = transition.evaluate(&json!({"state": off, "event": {"ordinal": u64::MAX, "outcome": {"backlight": {"completed": {"high": false}}}}}));
    assert_eq!(terminal["phase"], 10);
    let mut off = initial;
    off["phase"] = json!(9);
    let terminal = transition.evaluate(&json!({"state": off, "event": {"ordinal": 0, "outcome": {"backlight": {"provider-lost": null}}}}));
    assert_eq!(terminal["phase"], 10);
    assert_eq!(terminal["failure"], 42);
}

#[test]
fn st7789_autonomous_source_expands_one_kernel_topology_with_distinct_contacts() {
    let entry = crate::protocol_boot::minipitft_source::entry().unwrap();
    let count = |kind| {
        entry
            .expanded()
            .expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == kind)
            .count()
    };
    assert_eq!(count(crate::digital_base::contract::DIGITAL_KIND), 2);
    assert_eq!(count(crate::spi_base::contract::SPI_KIND), 1);
    assert_eq!(count("machine/clock/at"), 1);
    for gear in entry.expanded().expanded.gears.iter().filter(|gear| {
        matches!(
            gear.kind_id.as_str(),
            "machine/digital/access" | "machine/spi/exchange" | "machine/clock/at"
        )
    }) {
        std::eprintln!(
            "ST7789 native {} {}",
            gear.kind_id.as_str(),
            gear.gear_id.as_str()
        );
        if gear.kind_id.as_str() == "machine/digital/access" {
            assert!(
                matches!(
                    gear.gear_id.as_str().rsplit('/').next(),
                    Some("backlight" | "command-data")
                ),
                "{}",
                gear.gear_id.as_str()
            );
        }
    }
    assert_eq!(entry.expanded().front.inputs().len(), 1);
    assert_eq!(entry.expanded().front.outputs().len(), 3);
    let schema = entry
        .input_schema(&conduit_core::port_id("chunks"))
        .unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = schema.shape() else {
        panic!("chunk Fore")
    };
    assert_eq!(
        fields.iter().map(|field| field.name()).collect::<Vec<_>>(),
        ["index", "wire"]
    );
}
