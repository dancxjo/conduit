//! CPU Source conformance, not physical display or installed Play acceptance.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType as Type,
    StructuredInfoTypeShape as T, StructuredInfoValue as Value, StructuredInfoValueShape as V,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use std::collections::BTreeMap;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

struct Fixture {
    inputs: BTreeMap<String, Type>,
    evaluators: BTreeMap<String, PreparedPortableExpressionEvaluator>,
}
#[test]
fn ht16k33_expression_preparation_reports_its_actual_host_heap_cost() {
    let (fixture, observation) = allocation_probe::observe(Fixture::new);
    assert_eq!(fixture.evaluators.len(), 5);
    eprintln!("HT16K33 expressions including retained input Types: {observation:?}");
    let programs = Fixture::programs();
    for (name, program) in &programs {
        let (_evaluator, cost) = allocation_probe::observe(|| {
            PreparedPortableExpressionEvaluator::new(program).unwrap()
        });
        if name == "ht16k33-transition" {
            // Host-side regression guard, not an MCU workspace grant. The old
            // duplicate projection scratch exceeded 26 KiB on this host.
            assert!(cost.peak_bytes < 24 * 1024, "{cost:?}");
        }
        eprintln!("{name} evaluator only: {cost:?}");
    }
}
impl Fixture {
    fn new() -> Self {
        let programs = Self::programs();
        let inputs = programs
            .iter()
            .map(|(name, program)| (name.clone(), program.input_type.clone()))
            .collect();
        let evaluators = programs
            .iter()
            .map(|(name, program)| {
                (
                    name.clone(),
                    PreparedPortableExpressionEvaluator::new(program).unwrap(),
                )
            })
            .collect();
        Self { inputs, evaluators }
    }
    fn programs() -> BTreeMap<String, PortableExpressionProgram> {
        let types = check_syntax_document(
            &parse_syntax_document(include_str!(
                "../../../plots/device-protocols/i2c-types.conduit"
            )),
            &StartupCatalog::new(),
        )
        .unwrap();
        let mut catalog = StartupCatalog::new();
        for (name, path) in [
            ("I2cTransaction", "machine/i2c/transact/request"),
            ("I2cResult", "machine/i2c/transact/result"),
        ] {
            catalog
                .insert_checked_native_type(
                    path,
                    types.native_types.iter().find(|t| t.name == name).unwrap(),
                )
                .unwrap();
        }
        let syntax = parse_syntax_document(include_str!(
            "../../../plots/device-protocols/ht16k33.conduit"
        ));
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &catalog).unwrap();
        let mut programs = BTreeMap::new();
        for plot in &checked.plots {
            let expanded =
                expand_canonical_plot_for_authoring(&checked, &plot.name, &ProfileCatalog::new())
                    .unwrap()
                    .expanded;
            assert_eq!(expanded.gears.len(), 1);
            let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
            else {
                panic!("program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            programs.insert(plot.name.clone(), program);
        }
        programs
    }
    fn evaluate(&mut self, name: &str, value: &Value) -> Value {
        let bytes = value.canonical_bytes().unwrap();
        let schema = self.inputs[name].canonical_bytes().unwrap();
        let (result, allocations) = allocation_probe::observe(|| {
            let validator =
                conduit_core::BorrowedStructuredValueValidator::new(&schema, bytes.len()).unwrap();
            let input = validator.validate(&bytes).unwrap();
            assert_eq!(validator.validate_node(input.value_node()).unwrap(), input);
            let result = self
                .evaluators
                .get_mut(name)
                .unwrap()
                .evaluate(&bytes)
                .unwrap();
            let output = conduit_core::validate_canonical_structured_value(result).unwrap();
            let output_validator = conduit_core::BorrowedStructuredValueValidator::new(
                output.type_bytes(),
                result.len(),
            )
            .unwrap();
            assert_eq!(
                output_validator.validate_node(output.value_node()).unwrap(),
                output
            );
            result
        });
        assert_eq!(allocations.allocations, 0, "{name}: {allocations:?}");
        assert_eq!(allocations.reallocations, 0, "{name}: {allocations:?}");
        Value::from_canonical_bytes(result).unwrap()
    }
    fn begin(&mut self, address: u8, brightness: u8, blink: u8, enabled: bool) -> Value {
        let ty = &self.inputs["ht16k33-initialize"];
        let value = record(
            ty,
            [
                ("address", scalar(member(ty, "address"), address)),
                ("brightness", scalar(member(ty, "brightness"), brightness)),
                ("blink", scalar(member(ty, "blink"), blink)),
                ("enabled", scalar(member(ty, "enabled"), u8::from(enabled))),
            ],
        );
        self.evaluate("ht16k33-initialize", &value)
    }
    fn step(&mut self, state: &Value, tag: &str, bytes: &[u8]) -> Value {
        let ty = &self.inputs["ht16k33-transition"];
        let result = outcome(member(ty, "result"), tag, bytes);
        let value = record(ty, [("state", state.clone()), ("result", result)]);
        self.evaluate("ht16k33-transition", &value)
    }
}
fn member<'a>(ty: &'a Type, name: &str) -> &'a Type {
    let T::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn scalar(ty: &Type, value: u8) -> Value {
    Value::leaf(ty.clone(), vec![value]).unwrap()
}
fn record<const N: usize>(ty: &Type, fields: [(&str, Value); N]) -> Value {
    Value::record(
        ty.clone(),
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let V::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn variant(value: &Value) -> (&str, &Value) {
    let V::Variant { tag, payload } = value.shape() else {
        panic!("variant")
    };
    (tag, payload)
}
fn octets(value: &Value) -> Vec<u8> {
    let V::Collection(values) = value.shape() else {
        panic!("octets")
    };
    values
        .iter()
        .map(|v| {
            let V::Leaf(b) = v.shape() else {
                panic!("octet")
            };
            b[0]
        })
        .collect()
}
fn bytes(ty: &Type, bytes: &[u8]) -> Value {
    let (element, sequence) = match ty.shape() {
        T::Collection { element, .. } => (element, false),
        T::Sequence { element, .. } => (element, true),
        _ => panic!("octets"),
    };
    let values = bytes.iter().map(|b| scalar(element, *b)).collect();
    if sequence {
        Value::sequence(ty.clone(), values).unwrap()
    } else {
        Value::collection(ty.clone(), values).unwrap()
    }
}
fn outcome(ty: &Type, tag: &str, input: &[u8]) -> Value {
    let T::Variant { cases, .. } = ty.shape() else {
        panic!("result")
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type();
    let payload = if matches!(tag, "completed" | "short") {
        record(payload, [("input", bytes(member(payload, "input"), input))])
    } else {
        Value::leaf(payload.clone(), vec![]).unwrap()
    };
    Value::variant(ty.clone(), tag, payload).unwrap()
}

#[test]
fn initialization_waits_for_each_ack_and_has_no_redraw_clear() {
    let mut f = Fixture::new();
    for address in [112, 119] {
        for brightness in 0..=15 {
            for blink in 0..=3 {
                for enabled in [false, true] {
                    let mut state = f.begin(address, brightness, blink, enabled);
                    for expected in [
                        vec![0x21],
                        vec![0x80],
                        vec![0; 17],
                        vec![0xe0 | brightness],
                        vec![0x80 | (blink << 1) | u8::from(enabled)],
                    ] {
                        let action = f.evaluate("ht16k33-action", &state);
                        let (tag, request) = variant(&action);
                        assert_eq!(tag, "transact");
                        assert_eq!(
                            field(request, "address"),
                            &scalar(field(request, "address").value_type(), address)
                        );
                        assert_eq!(octets(field(request, "write")), expected);
                        assert_eq!(
                            field(request, "read"),
                            &scalar(field(request, "read").value_type(), 0)
                        );
                        state = f.step(&state, "completed", &[]);
                    }
                    assert_eq!(
                        variant(&f.evaluate("ht16k33-action", &state)).0,
                        "acknowledged"
                    );
                    assert_eq!(f.step(&state, "provider-lost", &[]), state);
                }
            }
        }
    }
    let ty = &f.inputs["ht16k33-frame-request"];
    let ram: Vec<u8> = (0..16).map(|n| n * 17).collect();
    let frame = record(
        ty,
        [
            ("address", scalar(member(ty, "address"), 112)),
            ("ram", bytes(member(ty, "ram"), &ram)),
        ],
    );
    let action = f.evaluate("ht16k33-frame-request", &frame);
    assert_eq!(variant(&action).0, "transact");
    assert_eq!(
        octets(field(variant(&action).1, "write")),
        [vec![0], ram].concat()
    );
}

#[test]
fn every_bus_refusal_is_terminal_at_every_phase_and_short_is_not_an_ack() {
    let mut f = Fixture::new();
    for phase in 0..5 {
        let mut state = f.begin(112, 15, 0, true);
        for _ in 0..phase {
            state = f.step(&state, "completed", &[]);
        }
        for tag in [
            "short",
            "device-error",
            "not-acknowledged",
            "arbitration-lost",
            "timed-out",
            "provider-lost",
            "unsupported",
            "stale-attachment",
            "refused",
        ] {
            let failed = f.step(&state, tag, &[]);
            let action = f.evaluate("ht16k33-action", &failed);
            let (tag_out, failure) = variant(&action);
            assert_eq!(tag_out, "refused");
            assert_eq!(variant(failure).0, "bus");
            assert_eq!(variant(variant(failure).1).0, tag);
            assert_eq!(f.step(&failed, "completed", &[]), failed);
        }
        let malformed = f.step(&state, "completed", &[1]);
        let action = f.evaluate("ht16k33-action", &malformed);
        assert_eq!(variant(variant(&action).1).0, "malformed");
    }
    for (address, brightness, blink, reason) in [
        (111, 0, 0, "unsupported-address"),
        (112, 16, 0, "invalid-brightness"),
        (112, 0, 4, "invalid-blink"),
        (112, 255, 0, "invalid-brightness"),
        (112, 0, 255, "invalid-blink"),
    ] {
        let state = f.begin(address, brightness, blink, true);
        let action = f.evaluate("ht16k33-action", &state);
        assert_eq!(variant(&action).0, "refused");
        assert_eq!(variant(variant(&action).1).0, reason);
    }
    for (tag, data, expected) in [
        ("completed", &[][..], "acknowledged"),
        ("completed", &[0][..], "refused"),
        ("short", &[][..], "refused"),
        ("provider-lost", &[][..], "refused"),
    ] {
        let value = outcome(&f.inputs["ht16k33-write-outcome"], tag, data);
        assert_eq!(
            variant(&f.evaluate("ht16k33-write-outcome", &value)).0,
            expected
        );
    }
    // A directly supplied state cannot bypass the command parameter checks.
    for (address, brightness, blink, phase, reason) in [
        (111, 0, 0, 0, "unsupported-address"),
        (112, 255, 0, 3, "invalid-brightness"),
        (112, 0, 255, 4, "invalid-blink"),
        (112, 0, 0, 255, "unexpected"),
    ] {
        let state = f.begin(address, brightness, blink, true);
        let state = record(
            state.value_type(),
            [
                ("configuration", field(&state, "configuration").clone()),
                ("phase", scalar(member(state.value_type(), "phase"), phase)),
                ("failure", field(&state, "failure").clone()),
            ],
        );
        let action = f.evaluate("ht16k33-action", &state);
        assert_eq!(variant(&action).0, "refused");
        assert_eq!(variant(variant(&action).1).0, reason);
    }
}
