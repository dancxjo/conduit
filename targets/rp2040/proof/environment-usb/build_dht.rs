//! Ordinary checked authored DHT Source, lowered once by the shared Make exporter.
use conduit_core::ConfigurationValue;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    MakeExpressionMaterial, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog,
};
use std::{fs, path::Path};
fn program(
    source: &str,
    name: &str,
) -> (
    conduit_plot::CheckedSyntaxDocument,
    PortableExpressionProgram,
) {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(hex) = &expanded.gears[0].configuration[0].value else {
        panic!("authored expression")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    (checked, program)
}
pub fn emit(out: &Path) {
    let source = fs::read_to_string("../../../../plots/device-protocols/dht11.conduit").unwrap();
    let (checked, p) = program(&source, "dht11-decode");
    let canonical = p.canonical_bytes().unwrap();
    let material = MakeExpressionMaterial::export(&p).unwrap();
    fs::write(out.join("dht11.program"), &canonical).unwrap();
    fs::write(out.join("dht11.make"), material.bytes()).unwrap();
    fs::write(
        out.join("dht11-input.type"),
        p.input_type.canonical_bytes().unwrap(),
    )
    .unwrap();
    fs::write(out.join("dht-material.rs"),format!("pub const PROGRAM_DIGEST:[u8;32]={:?};\npub const MATERIAL_DIGEST:[u8;32]={:?};\npub const INPUT_DIGEST:[u8;32]={:?};\npub const BOUNDS:(u32,u32)=({},{});",conduit_core::assigned_plan_payload_digest(&canonical),conduit_core::assigned_plan_payload_digest(material.bytes()),conduit_core::assigned_plan_payload_digest(&p.input_type.canonical_bytes().unwrap()),p.maximum_prepared_input_bytes().unwrap(),p.maximum_prepared_output_bytes().unwrap())).unwrap();
    let forms = conduit_plot::rust_binding::generate_rust_bindings_with_forms(
        &[],
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    fs::write(out.join("forms.rs"), forms.source).unwrap();
    let source = ["pulse-types.conduit", "dht11-capture.conduit"]
        .map(|f| fs::read_to_string(format!("../../../../plots/device-protocols/{f}")).unwrap())
        .join("\n");
    let (_, p) = program(&source, "dht11-capture-request");
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let request = evaluator.evaluate(&[]).unwrap();
    fs::write(out.join("capture-request.info"), request).unwrap();
    let canonical = p.canonical_bytes().unwrap();
    fs::write(out.join("capture.program"), &canonical).unwrap();
    fs::write(
        out.join("capture-material.rs"),
        format!(
            "pub const PROGRAM_DIGEST:[u8;32]={:?};\npub const REQUEST_DIGEST:[u8;32]={:?};",
            conduit_core::assigned_plan_payload_digest(&canonical),
            conduit_core::assigned_plan_payload_digest(request)
        ),
    )
    .unwrap();
}
