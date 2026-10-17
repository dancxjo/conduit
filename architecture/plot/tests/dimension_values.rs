use conduit_core::{Dimension, InfoBool, StructuredInfoValueShape, Unit, DIMENSION_INFO_ID};
use conduit_plot::{
    check_syntax_document, checked_physical_catalog_for_document,
    expand_canonical_plot_for_authoring, parse_checked_physical_value, parse_syntax_document,
    CanonicalStartupValue, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog,
};

#[path = "common/generated_rust.rs"]
mod generated_rust;

const DECLARATIONS: &str = "dimension velocity = travel / clock\ndimension velocity2 = { travel: 1, clock: -1 }\ndimension travel\ndimension clock\ntype Motion = quantity { dimension: velocity }\nunit stepper : Motion = { reference: origin, scale: 1 }\nunit doublestep : Motion = { reference: stepper, scale: 2 }";

fn dimension(catalog: &StartupCatalog, name: &str) -> Dimension {
    let Some(CanonicalStartupValue::Structured(value)) =
        parse_checked_physical_value(name, None, catalog).unwrap()
    else {
        panic!("Dimension is an ordinary checked value");
    };
    let concrete = value.try_concrete().unwrap();
    let StructuredInfoValueShape::Leaf(bytes) = concrete.shape() else {
        panic!("leaf");
    };
    Dimension::decode(bytes).unwrap()
}
fn program(
    declarations: &str,
    input: &str,
    output: &str,
    expression: &str,
) -> PortableExpressionProgram {
    let source = format!("{declarations}\nplot specimen (\n >> input: {input} <= 8192B\n output: {output} <= 8192B >>\n) {{\n input >> ({expression}) >> output\n}}.\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "specimen", &ProfileCatalog::new()).unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("expression program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}

#[test]
fn source_basis_and_composed_dimensions_are_values_with_normalized_equality() {
    let catalog = checked_physical_catalog_for_document(
        &parse_syntax_document(DECLARATIONS),
        &StartupCatalog::new(),
    )
    .unwrap();
    let velocity = dimension(&catalog, "velocity");
    assert_eq!(velocity, dimension(&catalog, "velocity2"));
    assert_eq!(
        velocity,
        dimension(&catalog, "travel")
            .quotient(dimension(&catalog, "clock"))
            .unwrap()
    );
    let Some(CanonicalStartupValue::Unit(unit)) =
        parse_checked_physical_value("doublestep", None, &catalog).unwrap()
    else {
        panic!("Unit");
    };
    assert_eq!(unit.value().dimension(), velocity.definition());
    let equality = program(DECLARATIONS, "Dimension", "Boolean", ". == velocity2");
    assert_eq!(
        equality.evaluate(&velocity.encode()).unwrap(),
        InfoBool::TRUE.encode()
    );
    let mut prepared = PreparedPortableExpressionEvaluator::new(&equality).unwrap();
    assert_eq!(
        prepared.evaluate(&velocity.encode()).unwrap(),
        InfoBool::TRUE.encode()
    );
    assert_eq!(
        prepared
            .evaluate(&dimension(&catalog, "travel").encode())
            .unwrap(),
        InfoBool::FALSE.encode()
    );
    assert!(prepared.evaluate(&Unit::Meter.encode()).is_err());
    let mut forged = velocity.encode();
    forged[2] ^= 1;
    assert!(prepared.evaluate(&forged).is_err());
}

#[test]
fn canonical_dimension_runtime_transport_has_exact_finite_storage() {
    let transport = program(DECLARATIONS, "Dimension", "Dimension", ".");
    assert_eq!(
        match transport.input_type.shape() {
            conduit_core::StructuredInfoTypeShape::Leaf(kind) => kind.as_str(),
            _ => panic!("Dimension leaf"),
        },
        DIMENSION_INFO_ID
    );
    assert_eq!(
        transport.maximum_prepared_input_bytes().unwrap(),
        conduit_core::DIMENSION_ENCODED_LEN as u32
    );
    assert_eq!(
        transport.maximum_prepared_output_bytes().unwrap(),
        conduit_core::DIMENSION_ENCODED_LEN as u32
    );
    let value = Dimension::basis("travel").unwrap().encode();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&transport).unwrap();
    assert_eq!(prepared.evaluate(&value).unwrap(), value);
    assert!(prepared.evaluate(&value[..value.len() - 1]).is_err());
}

#[test]
fn dimension_declaration_cycles_unknowns_and_bounds_refuse_with_source_spans() {
    for (source, diagnostic) in [
        ("dimension a = b\ndimension b = a", "cycle"),
        ("dimension a = missing", "unknown"),
        ("dimension a\ndimension a", "duplicate"),
        ("dimension a\ndimension b = { a: 17 }", "bounded"),
        ("dimension a\ndimension b = a + a", "dimension laws"),
        ("dimension a\ndimension b = { a: 0 }", "bounded"),
    ] {
        let syntax = parse_syntax_document(source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let refusal =
            checked_physical_catalog_for_document(&syntax, &StartupCatalog::new()).unwrap_err();
        assert!(
            refusal.message.contains(diagnostic),
            "{source}: {refusal:?}"
        );
        assert!(refusal.span.end <= source.len());
        assert!(refusal.span.start < refusal.span.end);
    }
}

#[test]
fn dimension_startup_forwarding_and_structured_fields_keep_the_triad_distinct() {
    use conduit_core::{kind_id, StructuredFieldType, StructuredInfoType};
    use conduit_plot::{KindSignature, StartupParameterSignature};
    let mut catalog = StartupCatalog::new();
    let physical = StructuredInfoType::record(
        kind_id("test/physical-triad@1"),
        vec![
            StructuredFieldType::new(
                "dimension",
                StructuredInfoType::leaf(kind_id(DIMENSION_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "unit",
                StructuredInfoType::leaf(kind_id(conduit_core::UNIT_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "quantity",
                StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID)).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    catalog
        .insert_structured_type("PhysicalTriad", physical)
        .unwrap();
    catalog
        .insert(KindSignature {
            kind: "test/triad".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "PhysicalTriad".into(),
                default: None,
            }],
        })
        .unwrap();
    let source = format!("{DECLARATIONS}\nplot specimen (\n dimension: Dimension = velocity\n coordinate: Unit = doublestep\n magnitude: Quantity = 3doublestep\n) {{\n triad = {{ dimension: dimension, unit: coordinate, quantity: magnitude }}\n sink: test/triad(value = triad)\n}}.\n");
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &catalog).unwrap();
    let defaults = &checked.plots[0].startup_parameters;
    assert!(matches!(
        defaults[0].default,
        Some(CanonicalStartupValue::Structured(_))
    ));
    assert!(matches!(
        defaults[1].default,
        Some(CanonicalStartupValue::Unit(_))
    ));
    assert!(matches!(
        defaults[2].default,
        Some(CanonicalStartupValue::Quantity(_))
    ));
    let CanonicalStartupValue::Structured(triad) =
        &checked.plots[0].gears[0].startup_bindings[0].value
    else {
        panic!("structured forwarding");
    };
    assert!(
        triad.try_concrete().is_none(),
        "forwarded parameters remain in the checked startup IR until expansion"
    );
    let direct = format!("{DECLARATIONS}\nplot direct {{\n sink: test/triad(value = {{ dimension: velocity, unit: doublestep, quantity: 3doublestep }})\n}}.\n");
    let checked_direct = check_syntax_document(&parse_syntax_document(&direct), &catalog).unwrap();
    let CanonicalStartupValue::Structured(triad) =
        &checked_direct.plots[0].gears[0].startup_bindings[0].value
    else {
        panic!("concrete triad");
    };
    let concrete = triad.try_concrete().unwrap();
    let StructuredInfoValueShape::Record(fields) = concrete.shape() else {
        panic!("record");
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields
        .iter()
        .find(|f| f.name() == "dimension")
        .unwrap()
        .value()
        .shape()
    else {
        panic!("Dimension field");
    };
    assert_eq!(
        Dimension::decode(bytes).unwrap(),
        Dimension::basis("travel")
            .unwrap()
            .quotient(Dimension::basis("clock").unwrap())
            .unwrap()
    );
    for (from, to) in [
        (
            "dimension: Dimension = velocity",
            "dimension: Dimension = doublestep",
        ),
        (
            "coordinate: Unit = doublestep",
            "coordinate: Unit = velocity",
        ),
        (
            "magnitude: Quantity = 3doublestep",
            "magnitude: Quantity = velocity",
        ),
    ] {
        let bad = source.replace(from, to);
        assert!(
            check_syntax_document(&parse_syntax_document(&bad), &catalog).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn generated_dimension_records_and_generic_parameters_execute_and_compile_no_std() {
    let source = "type Holder<T> = {\n value: T\n}\ntype DimensionHolder = Holder<Dimension>\ntype PhysicalTriad = {\n dimension: Dimension\n unit: Unit\n quantity: Quantity\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let generated = conduit_plot::rust_binding::generate_rust_bindings(
        &checked.native_types,
        &conduit_plot::rust_binding::RustBindingOptions::default(),
    )
    .unwrap();
    generated_rust::exercise(
        &generated.source,
        r#"
#[cfg(test)]
mod acceptance {
    use super::*;
    #[test]
    fn dimension_generic_holder_round_trips() {
        let value = conduit_core::Dimension::basis("travel").unwrap();
        let holder = DimensionHolder::new(value).unwrap();
        let bytes = holder.clone().encode().unwrap();
        assert_eq!(DimensionHolder::decode(&bytes).unwrap(), holder);
        assert_eq!(*holder.value(), value);
    }
    #[test]
    fn triad_uses_distinct_native_carriers_and_rejects_corrupt_dimension() {
        let dimension = conduit_core::Dimension::from_definition(conduit_core::Unit::Meter.dimension());
        let triad = PhysicalTriad::new(dimension, conduit_core::Quantity::new(3, conduit_core::Unit::Meter), conduit_core::Unit::Meter).unwrap();
        let bytes = triad.clone().encode().unwrap();
        assert_eq!(PhysicalTriad::decode(&bytes).unwrap(), triad);
        let mut forged = dimension.encode();
        forged[2] ^= 1;
        assert!(StructuredInfoValue::leaf(StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::DIMENSION_INFO_ID)).unwrap(), forged.to_vec()).is_err());
        assert!(<conduit_core::Dimension as conduit_plot::rust_binding::NativePrimitive>::decode_primitive(&forged).is_none());
    }
}
"#,
        2,
        "dimension-binding",
    );
}

#[test]
fn later_declaration_admission_cannot_rebind_a_retained_dimension_program() {
    let original = "dimension travel\ndimension clock\ndimension velocity = travel / clock";
    let replacement = "dimension travel\ndimension clock\ndimension velocity = travel * clock";
    let original_catalog = checked_physical_catalog_for_document(
        &parse_syntax_document(original),
        &StartupCatalog::new(),
    )
    .unwrap();
    let replacement_catalog = checked_physical_catalog_for_document(
        &parse_syntax_document(replacement),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert!(checked_physical_catalog_for_document(
        &parse_syntax_document(replacement),
        &original_catalog
    )
    .is_err());
    let retained = program(original, "Dimension", "Boolean", ". == velocity");
    let before = dimension(&original_catalog, "velocity");
    let after = dimension(&replacement_catalog, "velocity");
    assert_ne!(before, after);
    assert_eq!(
        retained.evaluate(&before.encode()).unwrap(),
        InfoBool::TRUE.encode()
    );
    assert_eq!(
        retained.evaluate(&after.encode()).unwrap(),
        InfoBool::FALSE.encode()
    );
    let serialized = retained.canonical_bytes().unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(
        &PortableExpressionProgram::from_canonical_bytes(&serialized).unwrap(),
    )
    .unwrap();
    assert_eq!(
        prepared.evaluate(&before.encode()).unwrap(),
        InfoBool::TRUE.encode()
    );
}

#[test]
fn dimension_reference_depth_and_synthesized_unit_collisions_are_admitted_explicitly() {
    let mut deep = String::from("dimension d0\n");
    for index in 1..40 {
        deep.push_str(&format!("dimension d{index} = d{}\n", index - 1));
    }
    // Forward chains exercise depth admission rather than cached earlier results.
    let deep = deep.lines().rev().collect::<Vec<_>>().join("\n");
    let refusal = checked_physical_catalog_for_document(
        &parse_syntax_document(&deep),
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert!(refusal.message.contains("depth/work"), "{refusal:?}");
    let collision = "dimension span\ndimension kspan\ntype SpanQuantity = quantity {dimension:span}\nunit spanunit : SpanQuantity = {reference:origin,scale:1,prefixes:si}\ndimension kspanunit";
    let refusal = checked_physical_catalog_for_document(
        &parse_syntax_document(collision),
        &StartupCatalog::new(),
    )
    .unwrap_err();
    assert!(refusal.message.contains("conflicts"), "{refusal:?}");
}
