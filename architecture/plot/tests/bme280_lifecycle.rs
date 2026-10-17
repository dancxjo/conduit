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
            .insert_checked_native_type(
                path,
                checked
                    .native_types
                    .iter()
                    .find(|ty| ty.name == name)
                    .unwrap(),
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
        .insert_checked_native_type(
            "machine/clock/at/result",
            clock
                .native_types
                .iter()
                .find(|ty| ty.name == "MonotonicClockResult")
                .unwrap(),
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
        let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        PreparedPortableExpressionEvaluator::new(&program).unwrap();
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
