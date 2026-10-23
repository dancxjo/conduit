//! Exact wire tests, not proof that arbitrary fixture contracts grant effects.
//! Compile as `graph_encoding::nested_oracle_tests` with the engine test entrance.
use super::outer::Nested;
use super::{GraphEncoding, Node};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_human::*;
use patchbay_graph::{FaceControl, FaceControlKind, FaceInteraction};
use serde::Serialize;

fn oracle<T: Serialize + ?Sized>(value: &T, node: Node<'_>) {
    let mut expected = vec![0; 262_144];
    let length = super::super::transport_codec::encode_unvalidated(value, &mut expected).unwrap();
    // encode_unvalidated always writes the exact outer transport envelope.
    let prefix = b"[2,\"CAPSEQ1\",";
    assert!(expected[..length].starts_with(prefix));
    assert_eq!(expected[length - 1], b']');
    let expected = &expected[prefix.len()..length - 1];
    let mut actual = vec![0; 262_144];
    let mut encoder = GraphEncoding::for_node();
    let mut terminal = false;
    let mut turns = 0;
    for _ in 0..262_144 {
        turns += 1;
        if encoder.step_node(node, &mut actual).unwrap() {
            terminal = true;
            break;
        }
    }
    assert!(
        terminal,
        "nested encoder did not terminate within fixture limit"
    );
    if expected.len() > 8192 {
        assert!(
            turns > 1,
            "large nested value completed in one protected entry"
        );
    }
    assert_eq!(&actual[..encoder.length()], expected);
}
fn nested<T: Serialize + ?Sized>(value: &T, node: Nested<'_>) {
    oracle(value, Node::Nested(node));
}
fn escaped() -> String {
    "quote\" slash\\ controls\u{0}\u{1}\u{8}\u{c}\n\r\t é λ 🦀\u{2028}\u{2029}".repeat(1024)
}
fn configuration(value: &ConfigurationValue) {
    nested(value, Nested::Configuration(value));
}

#[test]
fn every_constraint_payload_option_endpoint_membership_and_pattern_matches_wire() {
    use IntervalEndpoint::{Exclusive as E, Inclusive as I};
    use ValueConstraint as C;
    let quantity = Quantity::new(-7, Unit::One);
    let pattern = CheckedTextPattern {
        states: vec![
            TextPatternState {
                accepting: false,
                transitions: vec![TextPatternTransition {
                    first_scalar: 0,
                    last_scalar: 0x10ffff,
                    target_state: 1,
                }],
            },
            TextPatternState {
                accepting: true,
                transitions: vec![],
            },
        ],
        start_state: 0,
        maximum_input_characters: 4096,
        maximum_match_steps: 8192,
    };
    let constraints = vec![
        C::ByteLength {
            minimum: 0,
            maximum: u32::MAX,
        },
        C::UnsignedRange {
            minimum: None,
            maximum: Some(u64::MAX),
            minimum_endpoint: I,
            maximum_endpoint: E,
        },
        C::UnsignedRange {
            minimum: Some(0),
            maximum: None,
            minimum_endpoint: E,
            maximum_endpoint: I,
        },
        C::SignedRange {
            minimum: Some(i64::MIN),
            maximum: Some(i64::MAX),
            minimum_endpoint: E,
            maximum_endpoint: I,
        },
        C::SignedRange {
            minimum: None,
            maximum: None,
            minimum_endpoint: I,
            maximum_endpoint: E,
        },
        C::FixedIntegerRange {
            minimum: Some(vec![0, 255, 128]),
            maximum: None,
            minimum_endpoint: I,
            maximum_endpoint: E,
        },
        C::FixedIntegerRange {
            minimum: None,
            maximum: Some(vec![]),
            minimum_endpoint: E,
            maximum_endpoint: I,
        },
        C::QuantityRange {
            minimum: Some(Box::new(quantity)),
            maximum: None,
            minimum_endpoint: E,
            maximum_endpoint: I,
        },
        C::QuantityRange {
            minimum: None,
            maximum: Some(Box::new(Quantity::new(9, Unit::Millisecond))),
            minimum_endpoint: I,
            maximum_endpoint: E,
        },
        C::FloatFinite,
        C::FloatRange {
            minimum: Some(vec![0, 0, 0, 128]),
            maximum: None,
            minimum_endpoint: I,
            maximum_endpoint: E,
        },
        C::FloatRange {
            minimum: None,
            maximum: Some(vec![255; 8]),
            minimum_endpoint: E,
            maximum_endpoint: I,
        },
        C::CanonicalMembership {
            members: vec![vec![], vec![0, 255], vec![42]],
            negated: false,
        },
        C::CanonicalMembership {
            members: vec![],
            negated: true,
        },
        C::TextPattern {
            pattern: pattern.clone(),
            anchored_start: true,
            anchored_end: false,
            negated: true,
        },
        C::TextPattern {
            pattern,
            anchored_start: false,
            anchored_end: true,
            negated: false,
        },
        C::NearQuantity {
            source_kind: kind_id("value/quantity@1"),
            limit: Box::new(quantity),
            schema: vec![0, 1, 127, 255],
        },
    ];
    // Public metadata is intentionally constructed directly: this is the wire
    // resolver oracle, independent of admission validity or constraint order.
    let contract = CheckedValueContract {
        value_kind: kind_id("wire/constraint-fixture"),
        maximum_bytes: u32::MAX,
        constraints,
    };
    nested(&contract, Nested::Contract(&contract));
    for constraint in &contract.constraints {
        nested(constraint, Nested::Constraint(constraint));
    }
    let empty = CheckedValueContract {
        value_kind: kind_id("wire/empty"),
        maximum_bytes: 0,
        constraints: vec![],
    };
    nested(&empty, Nested::Contract(&empty));
}

#[test]
fn configuration_integer_thresholds_extrema_and_escaped_text_match_wire() {
    const SAFE: u64 = 9_007_199_254_740_991;
    for value in [0, 1, SAFE - 1, SAFE, SAFE + 1, u64::MAX] {
        configuration(&ConfigurationValue::U64(value));
    }
    for value in [
        i64::MIN,
        -(SAFE as i64) - 1,
        -(SAFE as i64),
        -1,
        0,
        SAFE as i64,
        SAFE as i64 + 1,
        i64::MAX,
    ] {
        configuration(&ConfigurationValue::I64(value));
    }
    for value in [false, true] {
        configuration(&ConfigurationValue::Bool(value));
    }
    configuration(&ConfigurationValue::Text(String::new()));
    configuration(&ConfigurationValue::Text(escaped()));
}

fn physical_configurations() -> Vec<ConfigurationValue> {
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let structured = StructuredInfoValue::leaf(ty.clone(), encode_count(7).to_vec()).unwrap();
    vec![
        ConfigurationValue::Quantity(QuantityConfigurationValue::parse("21ms").unwrap()),
        ConfigurationValue::Unit(UnitConfigurationValue::parse("um").unwrap()),
        ConfigurationValue::TemperatureDifference(
            ExactTemperatureDifferenceConfigurationValue::parse("TemperatureDelta(21, °C)")
                .unwrap(),
        ),
        ConfigurationValue::Structured(
            StructuredConfigurationValue::new(
                ty.profile().unwrap().value_kind().clone(),
                structured.canonical_bytes().unwrap(),
            )
            .unwrap(),
        ),
    ]
}
#[test]
fn all_physical_and_structured_configuration_variants_match_retained_wire() {
    for value in physical_configurations() {
        configuration(&value);
    }
}

#[test]
fn every_control_kind_and_optional_interaction_matches_wire() {
    let kinds = vec![
        FaceControlKind::BooleanChoice {
            choices: ["false", "true"],
        },
        FaceControlKind::TextChoice { choices: vec![] },
        FaceControlKind::TextChoice {
            choices: vec![escaped(), String::new()],
        },
        FaceControlKind::Number {
            minimum: 0,
            maximum: u64::MAX,
            unit: None,
        },
        FaceControlKind::Number {
            minimum: 1,
            maximum: 2,
            unit: Some("µs\"\\".into()),
        },
        FaceControlKind::ScalarNumber {
            minimum: i64::MIN,
            maximum: i64::MAX,
            unit: "µ",
        },
        FaceControlKind::Range {
            minimum: 0,
            maximum: u64::MAX,
            unit: None,
        },
        FaceControlKind::Range {
            minimum: 1,
            maximum: 2,
            unit: Some("ms".into()),
        },
        FaceControlKind::StructuredSource,
        FaceControlKind::PhysicalSource {
            maximum_bytes: u32::MAX,
        },
        FaceControlKind::ShortText { maximum_bytes: 0 },
    ];
    for kind in kinds {
        nested(&kind, Nested::ControlKind(&kind));
        let control = FaceControl {
            key: "key\"\\λ🦀".into(),
            value: ConfigurationValue::U64(u64::MAX),
            kind,
            interaction: None,
        };
        nested(&control, Nested::Control(&control));
    }
    let contract = InteractionContract::new("wire/boolean", InteractionFamily::Boolean).unwrap();
    let current =
        InteractionValue::new(kind_id(BOOL_INFO_ID), InfoBool::TRUE.encode().to_vec()).unwrap();
    let state = InteractionCurrentState::new(&contract, 0, None, vec![current]).unwrap();
    let control = FaceControl {
        key: "boolean".into(),
        value: ConfigurationValue::Bool(true),
        kind: FaceControlKind::BooleanChoice {
            choices: ["false", "true"],
        },
        interaction: Some(FaceInteraction { contract, state }),
    };
    nested(&control, Nested::Control(&control));
    for value in physical_configurations() {
        let control = FaceControl {
            key: "physical".into(),
            value,
            kind: FaceControlKind::PhysicalSource { maximum_bytes: 384 },
            interaction: None,
        };
        nested(&control, Nested::Control(&control));
    }
}

#[test]
fn every_generated_family_and_availability_matches_exact_generated_serde() {
    let families = vec![
        InteractionFamily::Activate,
        InteractionFamily::Boolean,
        InteractionFamily::choice_one(kind_id("wire/options"), 3),
        InteractionFamily::choice_many(kind_id("wire/options"), 4, 1, 3),
        InteractionFamily::scalar_range(
            Unit::Millisecond,
            -7,
            BoundKind::Inclusive,
            9,
            BoundKind::Exclusive,
            1,
        ),
        InteractionFamily::relative_range(Unit::One, -9, 7, 1),
        InteractionFamily::text_value(65536, true),
        InteractionFamily::text_value(1, false),
        InteractionFamily::structured_value(kind_id("wire/structured"), [255; 32], 65536),
    ];
    let value = InteractionValue::new(kind_id("wire/options"), vec![0, 1, 127, 255]).unwrap();
    nested(&value, Nested::Value(&value));
    for family in families {
        nested(&family, Nested::Family(&family));
        let contract = InteractionContract::new("wire/family", family).unwrap();
        nested(&contract, Nested::InteractionContract(&contract));
        // Wire fixtures deliberately preserve all plain struct branches without
        // claiming these arbitrary Current values were admitted for each family.
        for availability in [
            OptionAvailability::Available,
            OptionAvailability::unavailable("blocked\"\\λ".into()).unwrap(),
        ] {
            nested(&availability, Nested::Availability(&availability));
            let domain = InteractionDomain {
                revision: u64::MAX,
                options: vec![InteractionOption {
                    identity: "option\"🦀".into(),
                    value: value.clone(),
                    availability,
                }],
            };
            nested(&domain, Nested::Domain(&domain));
            let state = InteractionCurrentState {
                state_identity: "state\"λ".into(),
                contract_identity: contract.contract_identity.clone(),
                revision: u64::MAX,
                domain: Some(domain),
                current: vec![value.clone()],
            };
            nested(&state, Nested::Current(&state));
            let interaction = FaceInteraction {
                contract: contract.clone(),
                state,
            };
            nested(&interaction, Nested::Interaction(&interaction));
        }
        let empty = InteractionCurrentState {
            state_identity: "empty".into(),
            contract_identity: contract.contract_identity,
            revision: 0,
            domain: None,
            current: vec![],
        };
        nested(&empty, Nested::Current(&empty));
        let empty_domain = InteractionDomain {
            revision: 0,
            options: vec![],
        };
        nested(&empty_domain, Nested::Domain(&empty_domain));
    }
}
