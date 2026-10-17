use conduit_core::{ConfigurationValue, StructuredInfoValue};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::NativeRustBinding, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
use conduit_speech::semantic::SpeechUtteranceIntent;
pub const SOURCE: &str = include_str!("../../../examples/linguistic/committed-transport.conduit");
pub fn execute(intent: &SpeechUtteranceIntent) -> SpeechUtteranceIntent {
    let mut startup = StartupCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    assert_eq!(checked.source_document_id, syntax.source_document_id());
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "committed-linguistic-transport",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(program) = &expanded.gears[0].configuration[0].value else {
        panic!("checked expression")
    };
    let mut evaluator = PreparedPortableExpressionEvaluator::new(
        &PortableExpressionProgram::from_canonical_hex(program).unwrap(),
    )
    .unwrap();
    let result = StructuredInfoValue::from_canonical_bytes(
        evaluator
            .evaluate(&intent.clone().encode().unwrap())
            .unwrap(),
    )
    .unwrap();
    SpeechUtteranceIntent::from_structured(result).unwrap()
}
