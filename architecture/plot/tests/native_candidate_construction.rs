use conduit_core::*;
use conduit_plot::*;
fn raw(ty: &StructuredInfoType) -> StructuredInfoType {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => raw(representation),
        StructuredInfoTypeShape::Leaf(kind) => StructuredInfoType::leaf(kind.clone()).unwrap(),
        StructuredInfoTypeShape::Collection { element, length } => {
            StructuredInfoType::collection(raw(element), Some(length)).unwrap()
        }
        StructuredInfoTypeShape::Record { schema, fields } => StructuredInfoType::record(
            schema.clone(),
            fields
                .iter()
                .map(|f| StructuredFieldType::new(f.name(), raw(f.value_type())).unwrap())
                .collect(),
        )
        .unwrap(),
        StructuredInfoTypeShape::Variant { schema, cases } => StructuredInfoType::variant(
            schema.clone(),
            cases
                .iter()
                .map(|c| StructuredVariantCase::new(c.tag(), raw(c.payload_type())).unwrap())
                .collect(),
        )
        .unwrap(),
        _ => panic!("fixture shape"),
    }
}
#[test]
fn distinct_bare_candidate_reconstruction_retains_zero_law_context_and_exact_startup_precision() {
    let source="type Finite = F32 finite\ntype Vector = collection Finite = 2\ntype Period = U16 in 32..=255\ntype Precision =\n a\n | b\ntype Receipt = collection U8 = 2\ntype Anchor = {\n bytes: Receipt\n precision: Precision\n}\ntype State = {\n values: Vector\n}\ntype Frame = {\n state: State\n period: Period\n anchor: Anchor\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let target = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "Frame")
        .unwrap()
        .value_type;
    let unrefined = raw(target);
    let StructuredInfoTypeShape::Record { fields, .. } = unrefined.shape() else {
        panic!("record")
    };
    let candidate =
        StructuredInfoType::record(kind_id("fixture/raw-candidate"), fields.to_vec()).unwrap();
    let mut startup = StartupCatalog::new();
    for ty in &checked.native_types {
        startup.insert_checked_native_type(&ty.name, ty).unwrap();
    }
    startup
        .insert_checked_native_type(
            "Candidate",
            &CheckedNativeType {
                name: "Candidate".into(),
                identity: kind_id("fixture/raw-candidate"),
                value_type: candidate,
                value_contracts: vec![],
                invariants: vec![],
            },
        )
        .unwrap();
    let plot="plot candidate (\n selected: Anchor = {bytes:[1,2],precision:a(\"\")}\n >> value: Frame\n result: Candidate >>\n) = {state:{values:[(.state.values.0 * 0x3f800000),(.state.values.1 * 0x3f800000)]},period:((.period - 32) + 32),anchor:{bytes:[(selected.bytes.0 + 0),(selected.bytes.1 + 0)],precision:selected.precision}}\n";
    let checked = check_syntax_document(&parse_syntax_document(plot), &startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, "candidate", &ProfileCatalog::new()).unwrap();
    let invalid = plot.replace("((.period - 32) + 32)", "(.period + 0)");
    let checked = check_syntax_document(&parse_syntax_document(&invalid), &startup).unwrap();
    let refusal =
        expand_canonical_plot_for_authoring(&checked, "candidate", &ProfileCatalog::new())
            .unwrap_err();
    assert_eq!(refusal.code, "CND-FRM-046");
    assert!(refusal.message.contains("FixedIntegerRange"));
}
