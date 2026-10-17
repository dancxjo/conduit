//! Ordinary authored fields and runtime connections use the canonical codecs.
use conduit_core::{
    validate_primitive_info, ConfigurationValue, Quantity, StructuredInfoTypeShape,
    StructuredInfoValue, StructuredInfoValueShape, Unit, QUANTITY_ENCODED_LEN, QUANTITY_INFO_ID,
    UNIT_ENCODED_LEN, UNIT_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};

#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn programs(value_type: &str) -> [PortableExpressionProgram; 3] {
    let source = format!(
        "type Envelope = {{\n value: {value_type}\n}}\n\
         plot pack (\n >> input: {value_type}\n output: Envelope >>\n) = ({{ value: . }})\n\
         plot relay (\n >> input: Envelope\n output: Envelope >>\n) {{\n input >> (.) >> output\n}}\n\
         plot unpack (\n >> input: Envelope\n output: {value_type} >>\n) = (.value)\n"
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    ["pack", "relay", "unpack"].map(|entry| {
        let expanded =
            expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
        let gear = &expanded.expanded.gears[0];
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("prepared ordinary expression")
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
    })
}

#[test]
fn quantity_and_unit_fields_cross_prepared_runtime_connections_without_growth() {
    let quantity = Quantity::parse_plot_literal("1Qm³").unwrap().encode();
    let unit = Unit::resolve("qm³").unwrap().encode();
    for (name, kind, bytes) in [
        ("Quantity", QUANTITY_INFO_ID, quantity.as_slice()),
        ("Unit", UNIT_INFO_ID, unit.as_slice()),
    ] {
        let [pack, relay, unpack] = programs(name);
        assert_eq!(pack.maximum_prepared_input_bytes(), Ok(bytes.len() as u32));
        assert_eq!(
            unpack.maximum_prepared_output_bytes(),
            Ok(bytes.len() as u32)
        );
        assert_eq!(pack.output_type, relay.input_type);
        assert_eq!(relay.output_type, unpack.input_type);
        let StructuredInfoTypeShape::Record { fields, .. } = pack.output_type.shape() else {
            panic!("declared record")
        };
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name(), "value");
        let StructuredInfoTypeShape::Leaf(field_kind) = fields[0].value_type().shape() else {
            panic!("primitive typed field")
        };
        assert_eq!(field_kind.as_str(), kind);

        let envelope = pack.evaluate(bytes).unwrap();
        let decoded = StructuredInfoValue::from_canonical_bytes(&envelope).unwrap();
        let StructuredInfoValueShape::Record(fields) = decoded.shape() else {
            panic!("structured runtime value")
        };
        assert_eq!(
            fields[0].value().shape(),
            StructuredInfoValueShape::Leaf(bytes)
        );

        let mut pack = PreparedPortableExpressionEvaluator::new(&pack).unwrap();
        let mut relay = PreparedPortableExpressionEvaluator::new(&relay).unwrap();
        let mut unpack = PreparedPortableExpressionEvaluator::new(&unpack).unwrap();
        let capacities = (
            pack.output_capacity(),
            relay.output_capacity(),
            unpack.output_capacity(),
        );
        let (correct, allocations) = allocation_probe::observe(|| {
            let mut correct = true;
            for _ in 0..1000 {
                let packed = pack.evaluate(bytes).unwrap();
                let relayed = relay.evaluate(packed).unwrap();
                correct &= relayed == envelope;
                correct &= unpack.evaluate(relayed).unwrap() == bytes;
            }
            correct
        });
        assert!(correct);
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
        assert_eq!(
            (
                pack.output_capacity(),
                relay.output_capacity(),
                unpack.output_capacity()
            ),
            capacities
        );
    }
}

#[test]
fn wrong_type_connections_and_forged_primitive_codecs_refuse() {
    for (input, output) in [("Unit", "Quantity"), ("Quantity", "Unit")] {
        let syntax = parse_syntax_document(&format!(
            "plot wrong (\n >> input: {input}\n output: {output} >>\n) {{\n input >> output\n}}\n"
        ));
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        assert!(check_syntax_document(&syntax, &StartupCatalog::new()).is_err());
    }
    let quantity = Quantity::parse_plot_literal("21°C").unwrap().encode();
    let unit = Unit::Celsius.encode();
    assert_eq!(quantity.len(), QUANTITY_ENCODED_LEN);
    assert_eq!(unit.len(), UNIT_ENCODED_LEN);
    assert!(validate_primitive_info(QUANTITY_INFO_ID, &unit).is_err());
    assert!(validate_primitive_info(UNIT_INFO_ID, &quantity).is_err());

    let [quantity_pack, _, _] = programs("Quantity");
    let [unit_pack, _, _] = programs("Unit");
    let mut quantity_pack = PreparedPortableExpressionEvaluator::new(&quantity_pack).unwrap();
    let mut unit_pack = PreparedPortableExpressionEvaluator::new(&unit_pack).unwrap();
    assert!(quantity_pack.evaluate(&unit).is_err());
    assert!(unit_pack.evaluate(&quantity).is_err());
    let mut malformed_quantity = quantity;
    malformed_quantity[0] = 2;
    assert!(quantity_pack.evaluate(&malformed_quantity).is_err());
    let mut malformed_unit = unit;
    malformed_unit[2] = 3; // The reviewed affine Celsius base cannot be prefixed.
    assert!(unit_pack.evaluate(&malformed_unit).is_err());
    assert_eq!(
        quantity_pack.evaluate(&quantity).unwrap(),
        programs("Quantity")[0].evaluate(&quantity).unwrap()
    );
    assert_eq!(
        unit_pack.evaluate(&unit).unwrap(),
        programs("Unit")[0].evaluate(&unit).unwrap()
    );
}
