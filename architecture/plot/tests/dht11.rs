//! The actual authored decoder, not an independent Rust sensor implementation.
use conduit_core::{
    ConfigurationValue, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn program() -> PortableExpressionProgram {
    let syntax = parse_syntax_document(include_str!(
        "../../../plots/device-protocols/dht11.conduit"
    ));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "dht11-decode", &ProfileCatalog::new())
            .unwrap()
            .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("checked portable expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}

fn capture(program: &PortableExpressionProgram, length: u8, wire: [u8; 5]) -> Vec<u8> {
    let ty = &program.input_type;
    let wire_type = field_type(ty, "wire");
    let StructuredInfoTypeShape::Collection { element, .. } = wire_type.shape() else {
        panic!("fixed octets")
    };
    let length = StructuredInfoValue::leaf(field_type(ty, "length").clone(), vec![length]).unwrap();
    let wire = StructuredInfoValue::collection(
        wire_type.clone(),
        wire.into_iter()
            .map(|b| StructuredInfoValue::leaf(element.clone(), vec![b]).unwrap())
            .collect(),
    )
    .unwrap();
    StructuredInfoValue::record(
        ty.clone(),
        vec![
            StructuredFieldValue::new("length", length).unwrap(),
            StructuredFieldValue::new("wire", wire).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn frame(humidity: u8, humidity_fraction: u8, temperature: u8, fraction: u8) -> [u8; 5] {
    let sum = humidity
        .wrapping_add(humidity_fraction)
        .wrapping_add(temperature)
        .wrapping_add(fraction);
    [humidity, humidity_fraction, temperature, fraction, sum]
}

fn variant(value: &StructuredInfoValue) -> (&str, &StructuredInfoValue) {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("typed result")
    };
    (tag, payload)
}

fn quantity(record: &StructuredInfoValue, name: &str) -> conduit_core::Quantity {
    let StructuredInfoValueShape::Record(fields) = record.shape() else {
        panic!("observation")
    };
    let value = fields.iter().find(|f| f.name() == name).unwrap().value();
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("fixed-point integer")
    };
    let coefficient = i128::from_le_bytes(bytes.try_into().unwrap());
    let syntax = parse_syntax_document(include_str!(
        "../../../plots/device-protocols/dht11.conduit"
    ));
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let form = checked
        .type_forms
        .iter()
        .find(|f| f.name == format!("device/dht11-{name}"))
        .unwrap();
    let conduit_plot::CheckedTypeFormStorage::DecimalQuantity(repr) = &form.storage else {
        panic!("quantity representation")
    };
    let bytes = match repr.integer {
        conduit_core::QuantityFormInteger::I16Le => {
            i16::try_from(coefficient).unwrap().to_le_bytes()
        }
        conduit_core::QuantityFormInteger::U16Le => {
            u16::try_from(coefficient).unwrap().to_le_bytes()
        }
    };
    repr.decode(&bytes).unwrap()
}

#[test]
fn published_example_fractional_values_and_negative_temperatures() {
    let program = program();
    for (wire, temperature, humidity) in [
        ([53, 0, 24, 0, 77], 2400, 5300),
        (frame(53, 2, 24, 7), 2470, 5320),
        (frame(53, 0, 10, 0x82), -1080, 5300),
        (frame(5, 0, 19, 0x80), -2000, 500),
        (frame(95, 0, 60, 0), 6000, 9500),
        // Sum exceeds 255: checksum is the low octet, without U8 overflow.
        (frame(95, 0, 19, 0x89), -1910, 9500),
    ] {
        let output = program.evaluate(&capture(&program, 5, wire)).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        let (tag, reading) = variant(&value);
        assert_eq!(tag, "frame", "{wire:?}");
        assert_eq!(
            quantity(reading, "temperature"),
            conduit_core::Quantity::from_decimal(temperature, -2, conduit_core::Unit::Celsius)
                .unwrap()
        );
        assert_eq!(
            quantity(reading, "humidity"),
            conduit_core::Quantity::from_decimal(humidity, -2, conduit_core::Unit::Percent)
                .unwrap()
        );
    }
}

#[test]
fn incomplete_bad_checksum_and_invalid_values_never_become_observations() {
    let program = program();
    let good = frame(53, 0, 24, 0);
    let mut cases = vec![
        (5, [53, 0, 24, 0, 73], "checksum"),
        (5, [0; 5], "invalid-payload"),
        (5, frame(96, 0, 24, 0), "invalid-payload"),
        (5, frame(95, 1, 24, 0), "invalid-payload"),
        (5, frame(53, 10, 24, 0), "invalid-payload"),
        (5, frame(53, 0, 24, 0x10), "invalid-payload"),
        (5, frame(53, 0, 24, 10), "invalid-payload"),
        (5, frame(53, 0, 60, 1), "invalid-payload"),
        (5, frame(53, 0, 61, 0), "invalid-payload"),
        (5, frame(53, 0, 20, 0x80), "invalid-payload"),
    ];
    cases.extend((0..5).map(|length| (length, good, "short-frame")));
    cases.extend([6, 255].map(|length| (length, good, "malformed-length")));
    for (length, wire, expected) in cases {
        let bytes = program.evaluate(&capture(&program, length, wire)).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let (tag, failure) = variant(&value);
        assert_eq!(tag, "unavailable");
        assert_eq!(variant(failure).0, expected, "{length}: {wire:?}");
    }
    // Every single-bit corruption of this complete frame is detected.
    for bit in 0..40 {
        let mut wire = good;
        wire[bit / 8] ^= 1 << (bit % 8);
        let bytes = program.evaluate(&capture(&program, 5, wire)).unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(variant(&value).0, "unavailable");
        assert_eq!(variant(variant(&value).1).0, "checksum");
    }
}

#[test]
fn prepared_decoder_reuses_storage_for_success_and_refusal() {
    let program = program();
    let success = capture(&program, 5, frame(53, 0, 24, 0));
    let bad_checksum = capture(&program, 5, [53, 0, 24, 0, 73]);
    let short = capture(&program, 0, [0; 5]);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    // Preparation and input encoding are outside the measured play boundary.
    for input in [&success, &bad_checksum, &short] {
        evaluator.evaluate(input).unwrap();
    }
    let ((), observation) = allocation_probe::observe(|| {
        for _ in 0..128 {
            for input in [&success, &bad_checksum, &short] {
                assert!(!evaluator.evaluate(input).unwrap().is_empty());
            }
        }
    });
    assert_eq!(observation.allocations, 0);
    assert_eq!(observation.reallocations, 0);
}

#[test]
fn decoder_preparation_has_a_bounded_host_footprint() {
    let encoded = program().canonical_bytes().unwrap();
    let (evaluator, footprint) = allocation_probe::observe(|| {
        let program = PortableExpressionProgram::from_canonical_bytes(&encoded).unwrap();
        PreparedPortableExpressionEvaluator::new(&program).unwrap()
    });
    // This measures the native test host, not the MCU's admitted heap profile.
    assert!(encoded.len() <= 16 * 1024);
    assert!(footprint.peak_bytes <= 128 * 1024, "{footprint:?}");
    assert!(footprint.requested_bytes <= 192 * 1024, "{footprint:?}");
    assert!(evaluator.output_capacity() <= 1024);
}
