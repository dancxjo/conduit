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

#[test]
fn startup_defaults_apply_nested_sequence_variant_laws() {
    let types = "type Override =\n type_body_px U16 in 8..=64\n | unchanged\ntype Local = {\n visual: sequence Override <= 29\n}\n";
    for value in [7, 18] {
        let source = format!(
            "{types}plot local (\n look: Local = {{visual: [type_body_px({value})]}}\n) {{\n}}\n"
        );
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new());
        if value == 18 {
            checked.unwrap();
        } else {
            let Err(error) = checked else {
                panic!("invalid nested default must refuse")
            };
            assert_eq!(error.code, "CND-FRM-058", "{error:?}");
            assert_eq!(
                &source[error.span.start..error.span.end],
                "{visual: [type_body_px(7)]}"
            );
        }
    }
}

#[test]
fn omitted_catalog_default_cannot_bypass_alias_native_laws() {
    for value in [
        "{low: 1, high: 2}",
        "{low: 0, high: 2}",
        "{low: 5, high: 2}",
    ] {
        let mut startup = catalog();
        startup
            .insert(KindSignature {
                kind: "fixture/default".into(),
                startup_parameters: vec![StartupParameterSignature {
                    name: "interval".into(),
                    value_type: "AIntervalShape".into(),
                    default: Some(value.into()),
                }],
            })
            .unwrap();
        let result = check_syntax_document(
            &parse_syntax_document("plot test {\n check: fixture/default()\n}\n"),
            &startup,
        );
        assert_eq!(
            result.is_ok(),
            value == "{low: 1, high: 2}",
            "default {value}"
        );
    }
}

#[test]
fn authored_default_keeps_owner_diagnostic_and_field_span() {
    let mut startup = catalog();
    let owner = check_syntax_document(
        &parse_syntax_document("type Interval = {\n low: U32 in 1..=10\n high: U32 in 1..=10\n where .low <= .high\n}\n"),
        &StartupCatalog::new(),
    ).unwrap();
    startup
        .insert_native_refinement_diagnostic(
            "fixture/interval@1",
            owner.native_types[0].value_type.clone(),
            owner_diagnostic,
        )
        .unwrap();
    let source = "plot test (\n interval: AIntervalShape = {low: 5, high: 2}\n) {\n}\n";
    let Err(error) = check_syntax_document(&parse_syntax_document(source), &startup) else {
        panic!("invalid owner default must refuse")
    };
    assert_eq!(error.code, "FIXTURE-INTERVAL");
    assert_eq!(&source[error.span.start..error.span.end], "2");
}
