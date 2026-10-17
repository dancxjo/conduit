use conduit_core::{
    Quantity, QuantityConversionRefusal, StructuredInfoValue, StructuredInfoValueShape, Unit,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    quantity_conversion, ProfileCatalog, StartupCatalog,
};

fn prepare(declarations: &str, source: &str, target: &str) -> StructuredInfoValue {
    let document = format!("{declarations}\nplot conversion (\n receipt: ExactQuantityConversionReceipt <= 8192B >>\n) {{\n convert: units/convert(source = {source}, to = {target})\n convert.receipt >> receipt\n}}.\n");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(&document);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    quantity_conversion::validate_source(&syntax, &checked).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "conversion", &profile).unwrap();
    let receipt =
        quantity_conversion::prepare_configuration(&expanded.expanded.gears[0].configuration)
            .unwrap();
    quantity_conversion::validate_receipt(&receipt).unwrap();
    receipt
}
fn field<'a>(record: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = record.shape() else {
        panic!("record")
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn bytes(value: &StructuredInfoValue) -> &[u8] {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
fn coordinate(receipt: &StructuredInfoValue) -> (i128, i16) {
    let StructuredInfoValueShape::Variant { tag, payload } = field(receipt, "result").shape()
    else {
        panic!("result")
    };
    assert_eq!(tag, "converted");
    (
        i128::from_le_bytes(bytes(field(payload, "coefficient")).try_into().unwrap()),
        i16::from_le_bytes(bytes(field(payload, "exponent")).try_into().unwrap()),
    )
}

#[test]
fn custom_unit_receipt_is_self_contained_and_old_values_keep_their_meaning() {
    let first = prepare(
        "unit smoot : Distance = { reference: m, scale: 1.7018 }",
        "1smoot",
        "m",
    );
    let second = prepare(
        "unit smoot : Distance = { reference: m, scale: 2 }",
        "1smoot",
        "m",
    );
    assert_eq!(coordinate(&first), (17018, -4));
    assert_eq!(coordinate(&second), (2, 0));
    let old = Quantity::decode(bytes(field(&first, "source"))).unwrap();
    let new = Quantity::decode(bytes(field(&second, "source"))).unwrap();
    assert_ne!(old.unit(), new.unit());
    assert_eq!(old.convert_to_unit(new.unit()), Ok((8509, -4)));
    quantity_conversion::validate_receipt(&first).unwrap();
}

#[test]
fn custom_quantity_family_and_compound_symbol_convert_without_a_rust_catalogue_entry() {
    let source = "dimension wobble\ntype Wobble = quantity { dimension: wobble }\nunit wob/s : Wobble = { reference: origin, scale: 1 }\nunit doublewob/s : Wobble = { reference: wob/s, scale: 2 }";
    let receipt = prepare(source, "3doublewob/s", "wob/s");
    assert_eq!(coordinate(&receipt), (6, 0));
    assert_eq!(bytes(field(&receipt, "original")), b"3doublewob/s");
    assert_eq!(bytes(field(&receipt, "target")), b"wob/s");
}

#[test]
fn decimal_and_binary_prefixes_are_independent_explicit_policies() {
    for (policy, source, expected) in [("si", "1kspan", (2, 3)), ("binary", "1Kispan", (2048, 0))] {
        let declaration =
            format!("unit span : Distance = {{ reference: m, scale: 2, prefixes: {policy} }}");
        assert_eq!(coordinate(&prepare(&declaration, source, "m")), expected);
    }
    let mut startup = StartupCatalog::new();
    quantity_conversion::install(&mut startup, &mut ProfileCatalog::new()).unwrap();
    for (policy, literal) in [
        ("none", "1Mfurlong"),
        ("none", "1Mifurlong"),
        ("si", "1Mifurlong"),
        ("binary", "1Mfurlong"),
    ] {
        let source = format!("unit furlong : Distance = {{ reference: m, scale: 201.168, prefixes: {policy} }}\nplot invalid {{\n convert: units/convert(source = {literal}, to = m)\n}}\n");
        assert!(
            check_syntax_document(&parse_syntax_document(&source), &startup).is_err(),
            "{policy}: {literal}"
        );
    }
}

#[test]
fn authored_point_and_delta_laws_share_the_unit_without_sharing_the_role() {
    let declarations = "dimension tick\ntype TickDelta = quantity { dimension: tick }\ntype TickPoint = quantity { point: TickDelta }\nunit originTick : TickPoint = { reference: origin, scale: 1, delta: { quantity: TickDelta, reference: origin, scale: 1 } }\nunit shiftedTick : TickPoint = { reference: originTick, scale: 2, offset: 10, delta: { quantity: TickDelta, reference: originTick, scale: 2 } }";
    let point = prepare(declarations, "3shiftedTick", "originTick");
    let delta = prepare(declarations, "TickDelta(3, shiftedTick)", "originTick");
    assert_eq!(coordinate(&point), (16, 0));
    assert_eq!(coordinate(&delta), (6, 0));
    let point = Quantity::decode(bytes(field(&point, "source"))).unwrap();
    let delta = Quantity::decode(bytes(field(&delta, "source"))).unwrap();
    assert_eq!(
        point.compare(delta),
        Err(QuantityConversionRefusal::IncompatibleQuantityRoles)
    );
}

#[test]
fn prefix_alias_evidence_does_not_require_an_ambient_registry() {
    let receipt = prepare(
        "unit span : Distance = { reference: m, scale: 2, prefixes: si }",
        "1uspan",
        "µspan",
    );
    assert_eq!(coordinate(&receipt), (1, 0));
    let source = Quantity::decode(bytes(field(&receipt, "source"))).unwrap();
    let target = Unit::decode(bytes(field(&receipt, "target-unit"))).unwrap();
    assert!(source.matches_literal_evidence("1uspan"));
    assert!(target.matches_source_evidence("µspan"));
    assert_eq!(source.convert_to_unit(target), Ok((1, 0)));
}
