#![cfg(feature = "semantic-bindings")]
//! Quoted Unicode is admitted through ordinary checked Source constructors.
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, highlight_syntax,
    parse_syntax_document, rust_binding::NativeRustBinding, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog, SyntaxHighlightKind,
};
use conduit_speech::semantic::SpeechPhoneNotation;

fn source(spelling: &str) -> String {
    include_str!("../examples/ipa/quoted-phone.conduit").replace("tʰ", spelling)
}

fn compile(source: &str, catalog: &StartupCatalog) -> Result<PortableExpressionProgram, String> {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), source);
    let checked = check_syntax_document(&syntax, catalog).map_err(|e| format!("{e:?}"))?;
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "quoted-phone", &ProfileCatalog::new())
            .map_err(|e| format!("{e:?}"))?;
    let conduit_core::ConfigurationValue::Text(hex) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("portable constructor")
    };
    PortableExpressionProgram::from_canonical_hex(hex).map_err(|e| format!("{e:?}"))
}

#[test]
fn quoted_phone_constructor_checks_unicode_and_preserves_tooling_and_native_identity() {
    let mut catalog = StartupCatalog::new();
    conduit_speech::authoring::install(&mut catalog).unwrap();
    for spelling in ["t͡ʃ", "d͡ʒ", "tʰ", "n̩", "ã", "ã"] {
        let source = source(spelling);
        let program = compile(&source, &catalog).unwrap();
        let bytes = program.evaluate(&[1]).unwrap();
        let phone = SpeechPhoneNotation::decode(&bytes).unwrap();
        assert_eq!(phone.spelling().get(), spelling);
        assert_eq!(phone.provenance().method(), "quoted IPA fixture");
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        assert_eq!(prepared.evaluate(&[1]).unwrap(), bytes);
        let quoted = format!("\"{spelling}\"");
        let strings = highlight_syntax(&source).unwrap();
        assert!(strings
            .iter()
            .any(|span| span.kind == SyntaxHighlightKind::String
                && source[span.start..span.end] == quoted));
    }
    for spelling in ["ch", "ax", "p_aspirated", "tʃ", "̃", "n̩ʰ", "tt", "ˈ"] {
        assert!(compile(&source(spelling), &catalog).is_err(), "{spelling}");
    }
}
