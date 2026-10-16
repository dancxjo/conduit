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
        let Err(error) = check_syntax_document(&parse_syntax_document(&source(value)), &startup)
        else {
            panic!("retained Native law must refuse");
        };
        assert_eq!(error.code, "CND-FRM-058");
        assert!(
            error.message.contains("native startup Type refinement"),
            "{}: {}",
            error.code,
            error.message
        );
    }
}

fn owner_diagnostic(
    _: &conduit_core::StructuredInfoValue,
    source: conduit_plot::NativeRefinementSource<'_>,
    _: &conduit_plot::rust_binding::NativeBindingRefusal,
) -> Option<conduit_plot::SyntaxCheckDiagnostic> {
    Some(conduit_plot::SyntaxCheckDiagnostic {
        code: "FIXTURE-INTERVAL",
        span: source.field_span("high")?,
        message: "interval order refused".into(),
    })
}

#[test]
fn compiled_owner_diagnostic_keeps_native_refusal_and_follows_immutable_field_alias() {
    let owner = check_syntax_document(
        &parse_syntax_document("type Interval = {\n low: U32 in 1..=10\n high: U32 in 1..=10\n where .low <= .high\n}\n"),
        &StartupCatalog::new(),
    ).unwrap();
    let mut startup = catalog();
    startup
        .insert_native_refinement_diagnostic(
            "fixture/interval@1",
            owner.native_types[0].value_type.clone(),
            owner_diagnostic,
        )
        .unwrap();
    let source =
        "plot test {\n high = 2\n verify: fixture/check(interval = {low: 5, high: high})\n}\n";
    let error = check_syntax_document(&parse_syntax_document(source), &startup).unwrap_err();
    assert_eq!(error.code, "FIXTURE-INTERVAL", "{error:?}");
    assert_eq!(&source[error.span.start..error.span.end], "2");
    let accepted = source.replace("low: 5", "low: 1");
    assert!(check_syntax_document(&parse_syntax_document(&accepted), &startup).is_ok());
    assert!(startup
        .insert_native_refinement_diagnostic(
            "fixture/other-owner@1",
            owner.native_types[0].value_type.clone(),
            owner_diagnostic,
        )
        .is_err());
}
