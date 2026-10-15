use conduit_plot::{
    check_syntax_document, parse_syntax_document, KindSignature, StartupCatalog,
    StartupParameterSignature,
};

fn catalog() -> StartupCatalog {
    let owner = check_syntax_document(&parse_syntax_document("type Interval = {\n low: U32 in 1..=10\n high: U32 in 1..=10\n where .low <= .high\n}\n"), &StartupCatalog::new()).unwrap();
    let ty = &owner.native_types[0];
    let mut startup = StartupCatalog::new();
    // An earlier shape-only alias must not hide the owner's retained laws.
    startup
        .insert_structured_type("AIntervalShape", ty.value_type.clone())
        .unwrap();
    startup.insert_checked_native_type("Interval", ty).unwrap();
    startup
        .insert(KindSignature {
            kind: "fixture/check".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "interval".into(),
                value_type: "AIntervalShape".into(),
                default: None,
            }],
        })
        .unwrap();
    startup
}

#[test]
fn explicit_startup_values_apply_the_owner_range_and_invariant() {
    let startup = catalog();
    let source =
        |value: &str| format!("plot test {{\n verify: fixture/check(interval = {value})\n}}\n");
    assert!(check_syntax_document(
        &parse_syntax_document(&source("{ low: 1, high: 2 }")),
        &startup
    )
    .is_ok());
    for value in ["{ low: 0, high: 2 }", "{ low: 5, high: 2 }"] {
        let error = check_syntax_document(&parse_syntax_document(&source(value)), &startup)
            .err()
            .expect("retained Native law must refuse");
        assert_eq!(error.code, "CND-FRM-058");
        assert!(
            error.message.contains("native startup Type refinement"),
            "{}: {}",
            error.code,
            error.message
        );
    }
}
