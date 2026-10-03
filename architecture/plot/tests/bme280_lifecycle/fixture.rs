use super::*;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};

pub struct Fixture {
    checked_plot_count: usize,
    transition: Vec<PreparedPortableExpressionEvaluator>,
    pub state_type: StructuredInfoType,
    event_type: StructuredInfoType,
    transition_type: StructuredInfoType,
    action: PreparedPortableExpressionEvaluator,
    initialize: PreparedPortableExpressionEvaluator,
    begin_type: StructuredInfoType,
    first: Vec<u8>,
    second: Vec<u8>,
}
impl Fixture {
    pub fn new() -> Self {
        let checked = check_syntax_document(
            &parse_syntax_document(include_str!(
                "../../../../plots/device-protocols/bme280-lifecycle.conduit"
            )),
            &lifecycle_catalog(),
        )
        .unwrap();
        let program = |name: &str| {
            let expanded =
                expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
                    .unwrap()
                    .expanded;
            let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
            else {
                panic!("program")
            };
            PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
        };
        let state_type = checked
            .native_types
            .iter()
            .find(|ty| ty.name == "BmeProtocolState")
            .unwrap()
            .value_type
            .clone();
        let event_type = checked
            .native_types
            .iter()
            .find(|ty| ty.name == "BmeProtocolEvent")
            .unwrap()
            .value_type
            .clone();
        let transition_type = checked
            .native_types
            .iter()
            .find(|ty| ty.name == "BmeProtocolTransition")
            .unwrap()
            .value_type
            .clone();
        let transition = [
            "bme280-step-begin",
            "bme280-step-address",
            "bme280-step-time",
            "bme280-step-clock",
            "bme280-step-bus",
            "bme280-step-probe-reset",
            "bme280-step-nvm-sleep",
            "bme280-step-humidity-config",
            "bme280-step-calibration",
            "bme280-step-humidity-conversion",
            "bme280-step-sample",
            "bme280-step-finish",
        ]
        .iter()
        .map(|name| PreparedPortableExpressionEvaluator::new(&program(name)).unwrap())
        .collect();
        let action =
            PreparedPortableExpressionEvaluator::new(&program("bme280-protocol-action")).unwrap();
        let initializer = program("bme280-protocol-initialize");
        let begin_type = initializer.input_type.clone();
        let initialize = PreparedPortableExpressionEvaluator::new(&initializer).unwrap();
        Self {
            checked_plot_count: checked.plots.len(),
            state_type,
            event_type,
            transition_type,
            transition,
            action,
            initialize,
            begin_type,
            first: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
            second: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }
    pub fn initial(&mut self) -> StructuredInfoValue {
        self.initial_address(0x76)
    }
    pub fn initial_address(&mut self, address: u8) -> StructuredInfoValue {
        let query = record(
            &self.begin_type,
            vec![(
                "address",
                leaf(field_type(&self.begin_type, "address"), &[address]),
            )],
        );
        StructuredInfoValue::from_canonical_bytes(
            self.initialize
                .evaluate(&query.canonical_bytes().unwrap())
                .unwrap(),
        )
        .unwrap()
    }
    pub fn tick(&self, now: u64) -> StructuredInfoValue {
        variant(&self.event_type, "tick", &now.to_le_bytes())
    }
    pub fn bus(&self, tag: &str, data: &[u8], now: u64) -> StructuredInfoValue {
        let timed_type = case_type(&self.event_type, "bus");
        let result_type = field_type(timed_type, "result");
        let payload = case_type(result_type, tag);
        let result = if matches!(payload.shape(), StructuredInfoTypeShape::Leaf(_)) {
            variant(result_type, tag, &[])
        } else {
            let input = field_type(payload, "input");
            let StructuredInfoTypeShape::Sequence { element, .. } = input.shape() else {
                panic!("sequence")
            };
            let bytes = StructuredInfoValue::sequence(
                input.clone(),
                data.iter().map(|byte| leaf(element, &[*byte])).collect(),
            )
            .unwrap();
            let value = record(payload, vec![("input", bytes)]);
            StructuredInfoValue::variant(result_type.clone(), tag, value).unwrap()
        };
        let timed = record(
            timed_type,
            vec![
                ("result", result),
                (
                    "now",
                    leaf(field_type(timed_type, "now"), &now.to_le_bytes()),
                ),
            ],
        );
        StructuredInfoValue::variant(self.event_type.clone(), "bus", timed).unwrap()
    }
    pub fn advance(
        &mut self,
        state: StructuredInfoValue,
        event: StructuredInfoValue,
    ) -> StructuredInfoValue {
        let bytes = record(
            &self.transition_type,
            vec![("state", state), ("event", event)],
        )
        .canonical_bytes()
        .unwrap();
        StructuredInfoValue::from_canonical_bytes(self.advance_bytes(&bytes)).unwrap()
    }
    pub fn advance_bytes(&mut self, input: &[u8]) -> &[u8] {
        assert!(input.len() <= self.first.capacity());
        self.first.clear();
        self.first.extend_from_slice(input);
        for evaluator in &mut self.transition {
            let output = evaluator.evaluate(&self.first).unwrap();
            assert!(output.len() <= self.second.capacity());
            self.second.clear();
            self.second.extend_from_slice(output);
            core::mem::swap(&mut self.first, &mut self.second);
        }
        &self.first
    }
    pub fn input_bytes(&self, state: StructuredInfoValue, event: StructuredInfoValue) -> Vec<u8> {
        record(
            &self.transition_type,
            vec![("state", state), ("event", event)],
        )
        .canonical_bytes()
        .unwrap()
    }

    pub fn action(&mut self, state: &StructuredInfoValue) -> StructuredInfoValue {
        StructuredInfoValue::from_canonical_bytes(
            self.action
                .evaluate(&state.canonical_bytes().unwrap())
                .unwrap(),
        )
        .unwrap()
    }
    pub fn with_phase(&self, state: &StructuredInfoValue, phase: u8) -> StructuredInfoValue {
        let StructuredInfoValueShape::Record(fields) = state.shape() else {
            panic!("state")
        };
        record(
            &self.state_type,
            fields
                .iter()
                .map(|field| {
                    (
                        field.name(),
                        if field.name() == "phase" {
                            leaf(field.value().value_type(), &[phase])
                        } else {
                            field.value().clone()
                        },
                    )
                })
                .collect(),
        )
    }
    pub fn checked_plot_count(&self) -> usize {
        self.checked_plot_count
    }
}
pub fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
}
fn case_type<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
}
fn leaf(ty: &StructuredInfoType, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), bytes.to_vec()).unwrap()
}
fn variant(ty: &StructuredInfoType, tag: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::variant(ty.clone(), tag, leaf(case_type(ty, tag), bytes)).unwrap()
}
fn record(
    ty: &StructuredInfoType,
    values: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.clone(),
        values
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
pub fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
pub fn byte(value: &StructuredInfoValue) -> u8 {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("byte")
    };
    bytes[0]
}
pub fn tag(value: &StructuredInfoValue) -> &str {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("variant")
    };
    tag
}
pub fn payload(value: &StructuredInfoValue) -> &StructuredInfoValue {
    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
        panic!("variant")
    };
    payload
}
pub fn bytes(value: &StructuredInfoValue) -> Vec<u8> {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!("collection")
    };
    values.iter().map(byte).collect()
}
