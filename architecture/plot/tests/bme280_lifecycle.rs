use conduit_core::ConfigurationValue;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
fn lifecycle_catalog() -> StartupCatalog {
    let checked = check_syntax_document(
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
            .insert_checked_native_type_from_document(
                path,
                &checked,
                &checked
                    .native_types
                    .iter()
                    .find(|ty| ty.name == name)
                    .unwrap()
                    .identity,
            )
            .unwrap();
    }
    let clock = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/clock-types.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    catalog
        .insert_checked_native_type_from_document(
            "machine/clock/at/result",
            &clock,
            &clock
                .native_types
                .iter()
                .find(|ty| ty.name == "MonotonicClockResult")
                .unwrap()
                .identity,
        )
        .unwrap();
    catalog
}
#[test]
fn lifecycle_policy_is_checked_source_and_prepares_without_device_code() {
    let syntax = parse_syntax_document(include_str!(
        "../../../plots/device-protocols/bme280-lifecycle.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &lifecycle_catalog()).unwrap();
    for name in checked.plots.iter().map(|plot| plot.name.as_str()) {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
            .unwrap_or_else(|error| panic!("{name}: {error:?}"))
            .expanded;
        if name == "bme280-protocol-transition" {
            assert!(expanded.gears.len() > 1);
        } else {
            assert_eq!(expanded.gears.len(), 1);
        }
        for gear in &expanded.gears {
            let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                panic!("program")
            };
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            PreparedPortableExpressionEvaluator::new(&program).unwrap();
        }
    }
}
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[path = "bme280_lifecycle/fixture.rs"]
mod fixture;
#[path = "bme280_lifecycle/transcript.rs"]
mod transcript;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn completed_status_advances_to_data_without_consuming_the_status_byte_twice() {
    use fixture::{byte, bytes, field, tag, Fixture};
    let mut fixture = Fixture::new();
    let initial = fixture.initial();
    let status = fixture.with_phase(&initial, 12);
    let sample_before = field(&status, "sample").clone();
    let status_event = fixture.bus("completed", &[0], 1);
    let data = fixture.advance(status, status_event);
    assert_eq!(byte(field(&data, "phase")), 13);
    assert_eq!(byte(field(&data, "index")), 0);
    assert_eq!(field(&data, "sample"), &sample_before);
    assert_eq!(tag(field(&data, "failure")), "none");
    // Only the next independently supplied bus result is a sample byte.
    let data_event = fixture.bus("completed", &[0x55], 2);
    let next = fixture.advance(data, data_event);
    assert_eq!(byte(field(&next, "phase")), 13);
    assert_eq!(byte(field(&next, "index")), 1);
    assert_eq!(
        bytes(field(&next, "sample")),
        vec![0x55, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(tag(field(&next, "failure")), "none");
}

#[test]
fn completed_calibration_block_does_not_consume_the_humidity_first_byte() {
    use fixture::{byte, bytes, field, tag, Fixture};
    let mut fixture = Fixture::new();
    let initial = fixture.initial();
    let block = fixture.with_phase(&initial, 7);
    let block = fixture.with_index(&block, 23);
    let block_event = fixture.bus("completed", &[0x55], 1);
    let humidity = fixture.advance(block, block_event);
    assert_eq!(byte(field(&humidity, "phase")), 8);
    assert_eq!(byte(field(&humidity, "index")), 0);
    assert_eq!(bytes(field(&humidity, "first"))[23], 0x55);
    assert_eq!(byte(field(&humidity, "humidity_first")), 0);
    assert_eq!(tag(field(&humidity, "failure")), "none");
    let humidity_event = fixture.bus("completed", &[0x66], 2);
    let next = fixture.advance(humidity, humidity_event);
    assert_eq!(byte(field(&next, "phase")), 9);
    assert_eq!(byte(field(&next, "index")), 0);
    assert_eq!(byte(field(&next, "humidity_first")), 0x66);
    assert_eq!(bytes(field(&next, "first"))[23], 0x55);
    assert_eq!(tag(field(&next, "failure")), "none");
}

#[test]
fn all_primitive_transition_stages_fit_the_original_static_constructor_limits() {
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/bme280-lifecycle.conduit"
        )),
        &lifecycle_catalog(),
    )
    .unwrap();
    let mut refusals = Vec::new();
    for name in checked
        .plots
        .iter()
        .map(|plot| plot.name.as_str())
        .filter(|name| name.starts_with("bme280-step-"))
    {
        let expanded =
            expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
        assert_eq!(expanded.expanded.gears.len(), 1);
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("program");
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        if let Err(error) = conduit_plot::generate_static_structured_expression(
            &checked,
            &program,
            conduit_plot::rust_binding::PreparedNativeBounds {
                canonical_bytes: 4096,
                scopes: 64,
                invariants: 64,
                contracts: 64,
            },
            4096,
        ) {
            refusals.push((name, error));
        }
    }
    assert!(
        refusals.is_empty(),
        "primitive static generation refusals: {refusals:?}"
    );
}

#[test]
fn preparation_footprint_is_finite_and_reported() {
    let (fixture, observation) = allocation_probe::observe(fixture::Fixture::new);
    assert!(fixture.checked_plot_count() > 10);
    assert!(observation.peak_bytes < 16 * 1024 * 1024, "{observation:?}");
    eprintln!("BME280 policy preparation: {observation:?}");
}

#[test]
fn automatic_clock_event_adapters_check_and_prepare_as_ordinary_source() {
    let mut catalog = lifecycle_catalog();
    let clock_types = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../plots/device-protocols/clock-types.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    catalog
        .insert_checked_native_type(
            "machine/clock/at/request",
            clock_types
                .native_types
                .iter()
                .find(|ty| ty.name == "MonotonicClockRequest")
                .unwrap(),
        )
        .unwrap();
    let lifecycle = include_str!("../../../plots/device-protocols/bme280-lifecycle.conduit");
    let events = include_str!("../../../plots/device-protocols/bme280-clock-events.conduit");
    let (imports, definitions) = events.split_once("type BmeClockContext").unwrap();
    let lifecycle = lifecycle.replace("with machine/clock/at/result as ClockResult", "");
    let source = format!("{imports}\n{lifecycle}\ntype BmeClockContext{definitions}");
    let checked = check_syntax_document(&parse_syntax_document(&source), &catalog).unwrap();
    for name in [
        "bme280-clock-bus-context",
        "bme280-clock-action-context",
        "bme280-clock-request",
        "bme280-clock-event",
    ] {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
            .unwrap()
            .expanded;
        assert_eq!(expanded.gears.len(), 1);
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("Source expression")
        };
        PreparedPortableExpressionEvaluator::new(
            &PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
        )
        .unwrap();
    }

    use conduit_core::{
        StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    };
    fn case<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
        let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
            panic!("variant")
        };
        cases
            .iter()
            .find(|case| case.tag() == tag)
            .unwrap()
            .payload_type()
    }
    fn variant(ty: &StructuredInfoType, tag: &str, bytes: &[u8]) -> StructuredInfoValue {
        StructuredInfoValue::variant(
            ty.clone(),
            tag,
            StructuredInfoValue::leaf(case(ty, tag).clone(), bytes.to_vec()).unwrap(),
        )
        .unwrap()
    }
    let evaluator = |name: &str| {
        let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
            .unwrap()
            .expanded;
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("Source expression")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        (
            program.input_type.clone(),
            PreparedPortableExpressionEvaluator::new(&program).unwrap(),
        )
    };
    let (context_type, mut request) = evaluator("bme280-clock-request");
    let waiting = variant(&context_type, "waiting", &123_u64.to_le_bytes());
    let encoded = waiting.canonical_bytes().unwrap();
    let output =
        StructuredInfoValue::from_canonical_bytes(request.evaluate(&encoded).unwrap()).unwrap();
    assert!(
        matches!(fixture::field(&output, "deadline").shape(), conduit_core::StructuredInfoValueShape::Leaf(bytes) if bytes == 123_u64.to_le_bytes())
    );
    let (completion_type, mut event) = evaluator("bme280-clock-event");
    let result_type = fixture::field_type(&completion_type, "result");
    let complete = |context: StructuredInfoValue, result: StructuredInfoValue| {
        StructuredInfoValue::record(
            completion_type.clone(),
            vec![
                StructuredFieldValue::new("context", context).unwrap(),
                StructuredFieldValue::new("result", result).unwrap(),
            ],
        )
        .unwrap()
    };
    let input = complete(
        waiting.clone(),
        variant(result_type, "completed", &125_u64.to_le_bytes()),
    )
    .canonical_bytes()
    .unwrap();
    let output =
        StructuredInfoValue::from_canonical_bytes(event.evaluate(&input).unwrap()).unwrap();
    assert_eq!(fixture::tag(&output), "event");
    assert_eq!(fixture::tag(fixture::payload(&output)), "tick");
    assert!(
        matches!(fixture::payload(fixture::payload(&output)).shape(), conduit_core::StructuredInfoValueShape::Leaf(bytes) if bytes == 125_u64.to_le_bytes())
    );
    for tag in [
        "unavailable",
        "provider-lost",
        "unsupported-deadline",
        "malformed",
        "timeout",
    ] {
        let refusal = variant(result_type, tag, &[]);
        let expected = refusal.canonical_bytes().unwrap();
        let input = complete(waiting.clone(), refusal)
            .canonical_bytes()
            .unwrap();
        let output =
            StructuredInfoValue::from_canonical_bytes(event.evaluate(&input).unwrap()).unwrap();
        assert_eq!(fixture::tag(&output), "failed");
        assert_eq!(
            fixture::payload(&output).canonical_bytes().unwrap(),
            expected
        );
    }

    for tag in ["not-acknowledged", "provider-lost", "refused"] {
        let bus_type = case(&context_type, "bus");
        let refusal = variant(bus_type, tag, &[]);
        let expected = refusal.canonical_bytes().unwrap();
        let bus_context =
            StructuredInfoValue::variant(context_type.clone(), "bus", refusal).unwrap();
        let request_input = bus_context.canonical_bytes().unwrap();
        let output =
            StructuredInfoValue::from_canonical_bytes(request.evaluate(&request_input).unwrap())
                .unwrap();
        assert!(
            matches!(fixture::field(&output, "deadline").shape(), conduit_core::StructuredInfoValueShape::Leaf(bytes) if bytes == 0_u64.to_le_bytes())
        );
        let input = complete(
            bus_context,
            variant(result_type, "completed", &125_u64.to_le_bytes()),
        )
        .canonical_bytes()
        .unwrap();
        let output =
            StructuredInfoValue::from_canonical_bytes(event.evaluate(&input).unwrap()).unwrap();
        let bus = fixture::payload(fixture::payload(&output));
        assert_eq!(
            fixture::field(bus, "result").canonical_bytes().unwrap(),
            expected
        );
        assert!(
            matches!(fixture::field(bus, "now").shape(), conduit_core::StructuredInfoValueShape::Leaf(bytes) if bytes == 125_u64.to_le_bytes())
        );
    }
}
