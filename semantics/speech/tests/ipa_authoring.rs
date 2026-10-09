#![cfg(feature = "semantic-bindings")]
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    rust_binding::{BoundedSequence, NativeRustBinding},
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_speech::semantic::*;

fn catalog() -> StartupCatalog {
    let mut catalog = StartupCatalog::new();
    conduit_speech::authoring::install(&mut catalog).unwrap();
    catalog
}

#[test]
fn authored_phone_literal_executes_as_the_same_native_domain_value() {
    let source = include_str!("../examples/ipa/phones-and-phonemes.conduit");
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &catalog()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "example/aspirated-t",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let [entry] = expanded.expanded.gears[0].configuration.as_slice() else {
        panic!("one pure authored phone expression")
    };
    let conduit_core::ConfigurationValue::Text(hex) = &entry.value else {
        panic!("retained authored program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let expected = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("phone/t-aspirated".into()).unwrap(),
        "tʰ".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let output = program
        .evaluate(&expected.clone().encode().unwrap())
        .unwrap();
    assert_eq!(SpeechPhone::decode(&output).unwrap(), expected);
}

#[test]
fn source_checker_refuses_phone_as_phoneme_and_phoneme_as_phone() {
    for (input, output) in [
        ("SpeechPhone", "SpeechPhoneme"),
        ("SpeechPhoneme", "SpeechPhone"),
    ] {
        let source = format!("plot invalid (\n >> input: {input}\n output: {output} >>\n) = (.)\n");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty());
        let checked = check_syntax_document(&syntax, &catalog()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "invalid", &ProfileCatalog::new())
                .is_err()
        );
    }
}

#[test]
fn authored_phoneme_keeps_explicit_phone_alternatives_and_optional_default() {
    let syntax = parse_syntax_document(include_str!("../examples/ipa/phones-and-phonemes.conduit"));
    let checked = check_syntax_document(&syntax, &catalog()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "example/phonemic-t", &ProfileCatalog::new())
            .unwrap();
    let conduit_core::ConfigurationValue::Text(hex) =
        &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    for default in [None, Some(PhoneId::new("phone/t".into()).unwrap())] {
        let expected = SpeechPhoneme::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            default,
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            PhonemeId::new("phoneme/t".into()).unwrap(),
            "t".into(),
            BoundedSequence::try_from_iter([
                PhoneId::new("phone/t".into()).unwrap(),
                PhoneId::new("phone/t-aspirated".into()).unwrap(),
            ])
            .unwrap(),
            SpeechSegmentStatus::Core,
        )
        .unwrap();
        let output = program
            .evaluate(&expected.clone().encode().unwrap())
            .unwrap();
        assert_eq!(SpeechPhoneme::decode(&output).unwrap(), expected);
    }
}
