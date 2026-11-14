use super::*;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};

pub struct Fixture {
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
        Self::from_source(include_str!(
            "../../../../plots/device-protocols/bme280-lifecycle.conduit"
        ))
    }
    pub fn staged_oracle() -> Self {
        Self::from_source(include_str!("staged_oracle.conduit"))
    }
    pub fn from_source(source: &str) -> Self {
        let syntax = parse_syntax_document(source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &lifecycle_catalog()).unwrap();
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
        let transition = transition_programs(&checked)
            .iter()
            .map(|program| PreparedPortableExpressionEvaluator::new(program).unwrap())
            .collect();
        let action =
            PreparedPortableExpressionEvaluator::new(&program("bme280-protocol-action")).unwrap();
        let initializer = program("bme280-protocol-initialize");
        let begin_type = initializer.input_type.clone();
        let initialize = PreparedPortableExpressionEvaluator::new(&initializer).unwrap();
        Self {
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
    pub fn clock_failure(&self, tag: &str) -> StructuredInfoValue {
        let clock_type = case_type(&self.event_type, "clock-failed");
        StructuredInfoValue::variant(
            self.event_type.clone(),
            "clock-failed",
            variant(clock_type, tag, &[]),
        )
        .unwrap()
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
        self.try_advance_bytes(input).unwrap()
    }
    pub fn try_advance_bytes(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_plot::PortableExpressionEvaluationRefusal> {
        assert!(input.len() <= self.first.capacity());
        self.first.clear();
        self.first.extend_from_slice(input);
        for evaluator in &mut self.transition {
            let output = evaluator.evaluate(&self.first)?;
            assert!(output.len() <= self.second.capacity());
            self.second.clear();
            self.second.extend_from_slice(output);
            core::mem::swap(&mut self.first, &mut self.second);
        }
        Ok(&self.first)
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
        self.with_state_byte(state, "phase", phase)
    }
    pub fn with_index(&self, state: &StructuredInfoValue, index: u8) -> StructuredInfoValue {
        self.with_state_byte(state, "index", index)
    }
    fn with_state_byte(
        &self,
        state: &StructuredInfoValue,
        name: &str,
        byte: u8,
    ) -> StructuredInfoValue {
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
                        if field.name() == name {
                            leaf(field.value().value_type(), &[byte])
                        } else {
                            field.value().clone()
                        },
                    )
                })
                .collect(),
        )
    }
    pub fn transition_gear_count(&self) -> usize {
        self.transition.len()
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

/// Follow exact authored binding/Cord order, never lexical Gear IDs or a list of
/// stage names. Refuse any shape beyond this test fixture's unary pure chain.
pub(super) fn transition_programs(
    checked: &conduit_plot::CheckedSyntaxDocument,
) -> Vec<PortableExpressionProgram> {
    let authored = expand_canonical_plot_for_authoring(
        checked,
        "bme280-protocol-transition",
        &ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(authored.input_bindings.len(), 1);
    assert_eq!(authored.output_bindings.len(), 1);
    let graph = &authored.expanded;
    let mut current = &authored.input_bindings[0].gear_id;
    let mut ordered = Vec::new();
    let mut programs = Vec::new();
    loop {
        assert!(!ordered.contains(current), "cyclic transition fixture");
        ordered.push(current.clone());
        let gear = graph
            .gears
            .iter()
            .find(|gear| &gear.gear_id == current)
            .unwrap();
        assert_eq!(gear.configuration.len(), 1);
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        assert_eq!(gear.configuration[0].key, "program");
        programs.push(PortableExpressionProgram::from_canonical_hex(encoded).unwrap());
        let outgoing: Vec<_> = graph
            .connections
            .iter()
            .filter(|cord| &cord.source_gear_id == current)
            .collect();
        if outgoing.is_empty() {
            assert_eq!(current, &authored.output_bindings[0].gear_id);
            break;
        }
        assert_eq!(outgoing.len(), 1, "transition fanout");
        current = &outgoing[0].sink_gear_id;
        assert_eq!(
            graph
                .connections
                .iter()
                .filter(|cord| &cord.sink_gear_id == current)
                .count(),
            1,
            "transition merge"
        );
    }
    assert_eq!(ordered.len(), graph.gears.len(), "disconnected transition");
    programs
}
