use super::*;
use lifecycle::{field_type, Fixture};
pub const SAMPLE: [u8; 8] = [0x65, 0x5a, 0xc0, 0x7e, 0xed, 0, 0x75, 0x30];
pub fn calibration() -> Vec<u8> {
    [
        27504_i32, 26435, -1000, 36477, -10685, 3024, 2855, 140, -7, 15500, -14600, 6000,
    ]
    .into_iter()
    .flat_map(|n| (n as u16).to_le_bytes())
    .collect()
}
pub fn number(value: &StructuredInfoValue) -> u64 {
    let conduit_core::StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("u64")
    };
    u64::from_le_bytes((*bytes).try_into().unwrap())
}
fn leaf(ty: &StructuredInfoType, n: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(ty.clone(), n.to_le_bytes().to_vec()).unwrap()
}
pub struct Sampling {
    pub protocol: Fixture,
    start: PreparedPortableExpressionEvaluator,
    start_type: StructuredInfoType,
    finish: PreparedPortableExpressionEvaluator,
    finish_type: StructuredInfoType,
    route: PreparedPortableExpressionEvaluator,
    refusal: PreparedPortableExpressionEvaluator,
    control: PreparedPortableExpressionEvaluator,
    transition_type: StructuredInfoType,
    idle: PreparedPortableExpressionEvaluator,
    resume: PreparedPortableExpressionEvaluator,
    restart: PreparedPortableExpressionEvaluator,
}
impl Sampling {
    pub fn new() -> Self {
        let source = format!(
            "{}\n{}",
            include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
            include_str!("../../../../plots/device-protocols/bme280-sampling.conduit")
        );
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &lifecycle_catalog()).unwrap();
        let make = |name| {
            let expanded =
                expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
                    .unwrap()
                    .expanded;
            assert_eq!(expanded.gears.len(), 1);
            let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
            else {
                panic!("program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            (
                program.input_type.clone(),
                PreparedPortableExpressionEvaluator::new(&program).unwrap(),
            )
        };
        // Every new primitive is checked and prepared, including routes not used
        // by this CPU transcript. Generic exchange composition is separate.
        for name in [
            "bme280-sampling-start",
            "bme280-sampling-reset-input",
            "bme280-sampling-finish",
            "bme280-sampling-route",
            "bme280-sampling-capture",
            "bme280-sampling-refusal",
            "bme280-sampling-idle",
            "bme280-sampling-control",
            "bme280-sampling-idle-step",
            "bme280-sampling-idle-resume",
            "bme280-sampling-restart",
        ] {
            let _ = make(name);
        }
        let (start_type, start) = make("bme280-sampling-start");
        let (finish_type, finish) = make("bme280-sampling-finish");
        let (_, route) = make("bme280-sampling-route");
        let (_, refusal) = make("bme280-sampling-refusal");
        let (transition_type, _) = make("bme280-sampling-idle-step");
        let (_, control) = make("bme280-sampling-control");
        let (_, idle) = make("bme280-sampling-idle-step");
        let (_, resume) = make("bme280-sampling-idle-resume");
        let (_, restart) = make("bme280-sampling-restart");
        Self {
            protocol: Fixture::new(),
            start,
            start_type,
            finish,
            finish_type,
            route,
            refusal,
            transition_type,
            control,
            idle,
            resume,
            restart,
        }
    }
    pub fn start(&mut self, count: u64, period: u64) -> StructuredInfoValue {
        self.start_address(count, period, 0x76)
    }
    pub fn start_address(&mut self, count: u64, period: u64, address: u8) -> StructuredInfoValue {
        let initialized = self.protocol.initial_address(address);
        let ty = field_type(&self.start_type, "configuration");
        let sensor_type = field_type(ty, "sensor");
        let sensor = record(
            sensor_type,
            vec![(
                "address",
                StructuredInfoValue::leaf(
                    field_type(sensor_type, "address").clone(),
                    vec![address],
                )
                .unwrap(),
            )],
        );
        let configuration = record(
            ty,
            vec![
                ("sensor", sensor),
                ("count", leaf(field_type(ty, "count"), count)),
                ("period_ms", leaf(field_type(ty, "period_ms"), period)),
            ],
        );
        let input = record(
            &self.start_type,
            vec![
                ("configuration", configuration),
                ("initialized", initialized),
            ],
        );
        evaluate(&mut self.start, &input)
    }
    pub fn advance(
        &mut self,
        state: StructuredInfoValue,
        event: StructuredInfoValue,
    ) -> StructuredInfoValue {
        if byte(field(&state, "terminal")) != 0 {
            return state;
        }
        if byte(field(&state, "waiting")) != 0 {
            let event_type = field_type(&self.transition_type, "event");
            let event = StructuredInfoValue::variant(
                event_type.clone(),
                tag(&event),
                payload(&event).clone(),
            )
            .unwrap();
            let transition = record(
                &self.transition_type,
                vec![("state", state.clone()), ("event", event)],
            );
            let restart = evaluate(&mut self.resume, &transition);
            return if tag(&restart) == "absent" {
                let outcome = evaluate(&mut self.idle, &transition);
                assert_eq!(tag(&outcome), "available");
                payload(&outcome).clone()
            } else {
                assert_eq!(tag(&restart), "available");
                let control = payload(&restart).clone();
                let address = byte(field(
                    field(field(&control, "configuration"), "sensor"),
                    "address",
                ));
                let initialized = self.protocol.initial_address(address);
                let input = record(
                    &self.finish_type,
                    vec![("control", control), ("protocol", initialized)],
                );
                evaluate(&mut self.restart, &input)
            };
        }
        // Active feedback holds only small control beside the advanced state.
        let control = payload(&evaluate(&mut self.control, &state)).clone();
        let advanced = self
            .protocol
            .advance(field(&state, "protocol").clone(), event);
        let input = record(
            &self.finish_type,
            vec![("control", control), ("protocol", advanced)],
        );
        evaluate(&mut self.finish, &input)
    }
    pub fn route(&mut self, state: &StructuredInfoValue) -> StructuredInfoValue {
        evaluate(&mut self.route, state)
    }
    pub fn refusal(&mut self, state: &StructuredInfoValue) -> StructuredInfoValue {
        let projected = evaluate(&mut self.refusal, state);
        assert_eq!(tag(&projected), "available");
        payload(&projected).clone()
    }
    pub fn acquire(
        &mut self,
        state: &mut StructuredInfoValue,
        mut now: u64,
        cycle: u8,
    ) -> StructuredInfoValue {
        let cal = calibration();
        let humidity = [0x6a, 1, 0, 0x13, 0x2b, 3, 30];
        let mut sample = SAMPLE;
        sample[7] += cycle;
        // Independent wire transcript: each full acquisition must probe/reset,
        // reread calibration, trigger conversion and read exactly eight bytes.
        let mut expected: Vec<(Vec<u8>, u8, Vec<u8>)> = vec![
            (vec![0xd0], 1, vec![0x60]),
            (vec![0xe0, 0xb6], 0, vec![]),
            (vec![0xf3], 1, vec![0]),
            (vec![0xf4, 0], 0, vec![]),
            (vec![0xf2, 1], 0, vec![]),
            (vec![0xf5, 0], 0, vec![]),
        ];
        expected.extend(
            cal.iter()
                .enumerate()
                .map(|(i, b)| (vec![0x88 + i as u8], 1, vec![*b])),
        );
        expected.push((vec![0xa1], 1, vec![75]));
        expected.extend(
            humidity
                .iter()
                .enumerate()
                .map(|(i, b)| (vec![0xe1 + i as u8], 1, vec![*b])),
        );
        expected.extend([(vec![0xf4, 0x25], 0, vec![]), (vec![0xf3], 1, vec![0])]);
        expected.extend(
            sample
                .iter()
                .enumerate()
                .map(|(i, b)| (vec![0xf7 + i as u8], 1, vec![*b])),
        );
        assert_eq!(expected.len(), 48);
        let mut requests = 0;
        let mut waits = 0;
        while byte(field(state, "captured")) == 0 {
            let action = self.protocol.action(field(state, "protocol"));
            let event = match tag(&action) {
                "transact" => {
                    let (write, read, response) = &expected[requests];
                    let request = payload(&action);
                    assert_eq!(byte(field(request, "address")), 0x76);
                    assert_eq!(bytes(field(request, "write")), *write);
                    assert_eq!(byte(field(request, "read")), *read);
                    requests += 1;
                    self.protocol.bus("completed", response, now)
                }
                "waiting" => {
                    waits += 1;
                    now = number(payload(&action));
                    self.protocol.tick(now)
                }
                other => panic!("unexpected acquisition action {other}"),
            };
            *state = self.advance(state.clone(), event);
            assert!(requests + waits <= 52);
        }
        assert_eq!(requests, 48);
        assert_eq!(waits, 2);
        field(state, "protocol").clone()
    }
}
fn evaluate(
    e: &mut PreparedPortableExpressionEvaluator,
    input: &StructuredInfoValue,
) -> StructuredInfoValue {
    StructuredInfoValue::from_canonical_bytes(
        e.evaluate(&input.canonical_bytes().unwrap()).unwrap(),
    )
    .unwrap()
}

pub struct Observation(Vec<PreparedPortableExpressionEvaluator>);
impl Observation {
    pub fn new() -> Self {
        let source = format!(
            "{}\n{}\n{}\n{}",
            include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
            include_str!("../../../../plots/device-protocols/bme280-compensation.conduit"),
            include_str!("../../../../plots/device-protocols/main.conduit"),
            include_str!("../../../../plots/device-protocols/bme280-capture-observation.conduit")
        );
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &lifecycle_catalog()).unwrap();
        let result = expand_canonical_plot_for_authoring(
            &checked,
            "bme280-observation-from-capture",
            &ProfileCatalog::new(),
        )
        .unwrap();
        let mut node = result.input_bindings[0].gear_id.clone();
        let mut stages = Vec::new();
        loop {
            let gear = result
                .expanded
                .gears
                .iter()
                .find(|g| g.gear_id == node)
                .unwrap();
            let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                panic!("program")
            };
            stages.push(
                PreparedPortableExpressionEvaluator::new(
                    &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                )
                .unwrap(),
            );
            if result.output_bindings.iter().any(|p| p.gear_id == node) {
                break;
            }
            let outgoing: Vec<_> = result
                .expanded
                .connections
                .iter()
                .filter(|c| c.source_gear_id == node)
                .collect();
            assert_eq!(outgoing.len(), 1);
            node = outgoing[0].sink_gear_id.clone();
        }
        assert_eq!(stages.len(), result.expanded.gears.len());
        Self(stages)
    }
    pub fn decode(&mut self, capture: StructuredInfoValue) -> StructuredInfoValue {
        self.0
            .iter_mut()
            .fold(capture, |value, stage| evaluate(stage, &value))
    }
}
