//! Explicit receipt replay; no model execution or played commitment is claimed.
use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::rust_binding::validate_native_invariants;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::path::PathBuf;

#[test]
#[ignore = "requires exact full-source root and actual protected rebase receipt"]
fn staged_full_origin_projection_and_exact_native_correlation() {
    let started = std::time::Instant::now();
    eprintln!("custody: full-source load start");
    let root = PathBuf::from(std::env::var("PARSER_CUSTODY_SOURCE_ROOT").unwrap());
    let build = std::fs::read_to_string(root.join("build.rs")).unwrap();
    let block = build
        .split("let source = [")
        .nth(1)
        .unwrap()
        .split("]\n    .join")
        .next()
        .unwrap();
    let mut source = String::new();
    for line in block.lines() {
        if let Some(tail) = line.split("include_str!(\"").nth(1) {
            let file = tail.split('"').next().unwrap();
            source.push_str(&std::fs::read_to_string(root.join(file)).unwrap());
            source.push('\n');
        }
    }
    source.push_str(include_str!("../parser_session_custody.conduit"));
    source.push_str(include_str!("../parser_session_custody_origins.conduit"));
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    eprintln!(
        "custody: full-source checked {}ms",
        started.elapsed().as_millis()
    );
    let row: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("PARSER_CUSTODY_REBASE_RECEIPT").unwrap()).unwrap(),
    )
    .unwrap();
    let read = |name: &str| {
        let bytes: Vec<u8> = serde_json::from_value(row[name].clone()).unwrap();
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
    };
    let context = read("rebase_context_bytes");
    let output = read("rebased_set_bytes");
    let origin = read("independent_admission_bytes");
    let ty = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap()
    };
    let record = |name: &str, fields: Vec<(&str, StructuredInfoValue)>| {
        let t = ty(name);
        let value = StructuredInfoValue::record(
            t.value_type.clone(),
            fields
                .into_iter()
                .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
                .collect(),
        )
        .unwrap();
        eprintln!(
            "custody origins: {name} Type={}B fullcanonical={}B start {}ms",
            t.value_type.canonical_bytes().unwrap().len(),
            value.canonical_bytes().unwrap().len(),
            started.elapsed().as_millis()
        );
        validate_native_invariants(&value, &t.invariants).unwrap();
        eprintln!(
            "custody origins: {name} admitted {}ms",
            started.elapsed().as_millis()
        );
        value
    };
    let previous = field(&context, "previous").clone();
    assert_eq!(&previous, field(&origin, "output"));
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "language-parser-protected-origin-edge",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let conduit_core::ConfigurationValue::Text(encoded) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    eprintln!(
        "custody: whole-origin Source projection start {}ms",
        started.elapsed().as_millis()
    );
    let projection = StructuredInfoValue::from_canonical_bytes(
        &program
            .evaluate(&origin.canonical_bytes().unwrap())
            .unwrap(),
    )
    .unwrap();
    let admission = field(&origin, "admission");
    let dep = field(field(field(admission, "fact"), "query"), "dependent");
    // Equality against every actual slot verifies the Source output; no host POS
    // or syntax selection is performed. This actual acquisition protects VOC2.
    assert_eq!(&projection, field(&previous, "edge2"));
    eprintln!(
        "custody: original={}B projection={}B dependent={dep:?}",
        origin.canonical_bytes().unwrap().len(),
        projection.canonical_bytes().unwrap().len()
    );
    let insertion = record(
        "LanguageParserProtectedInsertReceipt",
        vec![
            (
                "previous",
                field(field(&origin, "insert"), "previous").clone(),
            ),
            ("origin", projection.clone()),
            ("output", previous.clone()),
        ],
    );
    let initial_type = ty("LanguageParserProtectedInitialReceipt");
    let initial = StructuredInfoValue::record(
        initial_type.value_type.clone(),
        vec![StructuredFieldValue::new("insertion", insertion.clone()).unwrap()],
    )
    .unwrap();
    assert!(matches!(
        validate_native_invariants(&initial, &initial_type.invariants),
        Err(conduit_plot::rust_binding::NativeBindingRefusal::ViolatedInvariant { index: 0 })
    ));
    eprintln!(
        "custody: already-active initial refused {}ms",
        started.elapsed().as_millis()
    );
    let receipt = record(
        "LanguageParserProtectedRebaseReceipt",
        vec![("context", context.clone()), ("output", output.clone())],
    );
    let prior = record(
        "LanguageParserProtectedOriginCorrelation",
        vec![
            ("origin", projection.clone()),
            ("current", previous.clone()),
        ],
    );
    let next = record(
        "LanguageParserProtectedOriginCorrelation",
        vec![("origin", projection.clone()), ("current", output.clone())],
    );
    let StructuredInfoValueShape::Record(fields) = projection.shape() else {
        panic!("record")
    };
    let foreign = StructuredInfoValue::record(
        projection.value_type().clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    if f.name() == "origin_basis" {
                        field(field(&context, "rebase"), "basis").clone()
                    } else {
                        f.value().clone()
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert_ne!(foreign, projection);
    let changed = StructuredInfoValue::record(
        ty("LanguageParserProtectedOriginCorrelation")
            .value_type
            .clone(),
        vec![
            StructuredFieldValue::new("origin", foreign).unwrap(),
            StructuredFieldValue::new("current", output).unwrap(),
        ],
    )
    .unwrap();
    assert!(validate_native_invariants(
        &changed,
        &ty("LanguageParserProtectedOriginCorrelation").invariants
    )
    .is_err());
    for value in [insertion, receipt, prior, next] {
        assert_eq!(
            value,
            StructuredInfoValue::from_canonical_bytes(&value.canonical_bytes().unwrap()).unwrap()
        );
    }
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
