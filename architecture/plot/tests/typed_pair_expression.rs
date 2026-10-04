//! Prepared typed temporal joins feed ordinary checked nested projections.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[test]
fn typed_join_retains_nested_source_meaning_without_runtime_allocation() {
    let word = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let packet = StructuredInfoType::record(
        kind_id("type/Packet@1"),
        vec![StructuredFieldType::new("count", word.clone()).unwrap()],
    )
    .unwrap();
    let input = StructuredInfoValue::record(
        packet.clone(),
        vec![StructuredFieldValue::new(
            "count",
            StructuredInfoValue::leaf(word.clone(), 16_u64.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let mut pair =
        PreparedTypedTuplePairEncoder::new(packet.clone(), input.len() as u32, word, 8).unwrap();
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("Joined", pair.value_type().clone())
        .unwrap();
    let syntax =
        parse_syntax_document("plot extract (\n query: Joined >> value: U64\n) = (.0.count + .1)");
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "extract", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("checked ordinary expression")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert_eq!(&program.input_type, pair.value_type());
    let mut expression = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let right = 26_u64.to_le_bytes();
    let (_, allocations) = allocation_probe::observe(|| {
        for _ in 0..1000 {
            assert_eq!(
                expression
                    .evaluate(pair.encode(&input, &right).unwrap())
                    .unwrap(),
                42_u64.to_le_bytes()
            );
        }
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    let mut opaque = PreparedTuplePairEncoder::new(
        packet.profile().unwrap().value_kind().clone(),
        input.len() as u32,
        kind_id("value/u64"),
        8,
    )
    .unwrap();
    assert!(expression
        .evaluate(opaque.encode(&input, &right).unwrap())
        .is_err());
}

#[test]
fn tuple_projection_refuses_out_of_range_and_named_or_forged_records() {
    let word = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let tuple = tuple_info_type(vec![word.clone(), word]).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = tuple.shape() else {
        panic!("canonical tuple")
    };
    let named = StructuredInfoType::record(kind_id("type/Named@1"), fields.to_vec()).unwrap();
    let forged = StructuredInfoType::record(
        kind_id("conduitese/anonymous-tuple-forged@1"),
        fields.to_vec(),
    )
    .unwrap();
    for (ty, projection) in [(tuple, ".2"), (named, ".0"), (forged, ".0")] {
        let mut startup = StartupCatalog::new();
        startup.insert_structured_type("Joined", ty).unwrap();
        let syntax = parse_syntax_document(&format!(
            "plot extract (\n query: Joined >> value: U64\n) = ({projection})"
        ));
        assert!(syntax.diagnostics.is_empty());
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "extract", &ProfileCatalog::new())
                .is_err()
        );
    }
}
