#![cfg(feature = "semantic-bindings")]
use conduit_core::ConfigurationValue;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, highlight_syntax,
    parse_syntax_document, rust_binding::NativeRustBinding, ProfileCatalog, StartupCatalog,
    SyntaxHighlightKind,
};
use conduit_speech::{ipa_constructors::*, semantic::*};

const PHONETIC: &str = include_str!("../examples/ipa/quoted-transcriptions.conduit");
const PHONEMIC: &str = include_str!("../examples/ipa/quoted-phonemic.conduit");
fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_speech::authoring::install(&mut startup).unwrap();
    install(&mut startup, &mut profile).unwrap();
    (startup, profile)
}
fn prepared(
    source: &str,
    constructor: IpaConstructor,
    catalogs: &(StartupCatalog, ProfileCatalog),
) -> PreparedIpaValue {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    assert_eq!(syntax.round_trip(), source);
    for token in highlight_syntax(source).unwrap() {
        assert!(source.get(token.start..token.end).is_some());
    }
    assert!(highlight_syntax(source).unwrap().iter().any(|token| {
        token.kind == SyntaxHighlightKind::String && source[token.start..token.end].contains("t͡ʃ")
    }));
    let checked = check_syntax_document(&syntax, &catalogs.0).unwrap();
    assert_eq!(syntax.source_document_id(), checked.source_document_id);
    for binding in &checked.plots[0].gears[0].startup_bindings {
        let fields = contract(constructor).configuration;
        let field = fields
            .iter()
            .find(|field| field.key == binding.name)
            .unwrap();
        conduit_plot::validate_startup_configuration(field, binding.value.clone())
            .unwrap_or_else(|error| panic!("{}: {error:?}", binding.name));
    }
    validate_source(&syntax, &checked).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, &syntax.plots[0].name.text, &catalogs.1)
            .unwrap();
    let gear = &expanded.expanded.gears[0];
    assert_eq!(gear.kind_id.as_str(), constructor.kind());
    assert!(gear
        .configuration
        .iter()
        .all(|entry| matches!(entry.value, ConfigurationValue::Structured(_))));
    let value = prepare_configuration(constructor, &gear.configuration).unwrap();
    assert_eq!(value.value_kind(), &gear.outputs[0].value_kind);
    value
}
#[test]
fn four_qualified_source_constructors_produce_distinct_native_values() {
    let catalogs = catalogs();
    let full = prepared(PHONETIC, IpaConstructor::Phonetic, &catalogs);
    let full = SpeechPhoneticTranscription::decode(full.bytes()).unwrap();
    assert_eq!(full.original(), "ˈt͡ʃãː.n̩");
    let phone = PHONETIC
        .replace("speech/phonetic-from-ipa", "speech/phone-from-ipa")
        .replace("SpeechPhoneticTranscription", "SpeechPhoneNotation")
        .replace("ˈt͡ʃãː.n̩", "t͡ʃ");
    let phone = prepared(&phone, IpaConstructor::Phone, &catalogs);
    assert_eq!(
        SpeechPhoneNotation::decode(phone.bytes())
            .unwrap()
            .spelling()
            .get(),
        "t͡ʃ"
    );
    assert!(SpeechPhoneticTranscription::decode(phone.bytes()).is_err());
    let phonemic = prepared(PHONEMIC, IpaConstructor::Phonemic, &catalogs);
    let phonemic = SpeechPhonemicTranscription::decode(phonemic.bytes()).unwrap();
    assert_eq!(phonemic.original(), "ˈt͡ʃaː");
    assert_eq!(phonemic.bindings().len(), 2);
    let phoneme = PHONEMIC
        .replace("speech/phonemic-from-ipa", "speech/phoneme-from-ipa")
        .replace("SpeechPhonemicTranscription", "SpeechPhonemeNotation")
        .replace("ˈt͡ʃaː", "t͡ʃ");
    let phoneme = prepared(&phoneme, IpaConstructor::Phoneme, &catalogs);
    let phoneme = SpeechPhonemeNotation::decode(phoneme.bytes()).unwrap();
    assert_eq!(phoneme.definition().identity().get(), "phoneme/ch");
    assert_eq!(phoneme.notation().original(), "/t͡ʃ/");
}
#[test]
fn source_refusals_locate_unicode_escapes_and_explicit_scope_fields() {
    let catalogs = catalogs();
    for (source, expected) in [
        (PHONETIC.replace("ˈt͡ʃãː.n̩", r"t͡ʃ\n"), r"\n"),
        (PHONETIC.replace("ˈt͡ʃãː.n̩", r#"t͡ʃ\""#), r#"\""#),
        (PHONETIC.replace("ˈt͡ʃãː.n̩", r"t͡ʃ\\"), r"\\"),
        (PHONETIC.replace("ˈt͡ʃãː.n̩", "t͡ʃ̃"), "̃"),
        (PHONETIC.replace("ˈt͡ʃãː.n̩", "ːt"), "ː"),
        (PHONETIC.replace("ˈt͡ʃãː.n̩", "t͡ʃ."), "."),
        (PHONEMIC.replace("ˈt͡ʃaː", "ˈt͡ʃa"), "a"),
        (PHONEMIC.replace("ˈt͡ʃaː", "t͡ʃː"), "ː"),
        (PHONETIC.replace("ˈt͡ʃãː.n̩", &"t͡ʃ".repeat(257)), "t͡ʃ"),
        (
            PHONEMIC.replacen(
                "inventory_id: \"inventory/test\"",
                "inventory_id: \"inventory/foreign\"",
                1,
            ),
            "\"inventory/foreign\"",
        ),
        (
            PHONEMIC.replacen(
                "identity: \"variety/test\"",
                "identity: \"variety/foreign\"",
                1,
            ),
            "{identity: \"variety/foreign\", language: \"language/test\"}",
        ),
        (
            PHONEMIC.replacen(
                "revision: \"revision/test\"",
                "revision: \"revision/foreign\"",
                1,
            ),
            "\"revision/foreign\"",
        ),
    ] {
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &catalogs.0).unwrap();
        let error = validate_source(&syntax, &checked).unwrap_err();
        assert_eq!(
            &source[error.span.start..error.span.end],
            expected,
            "{error:?}"
        );
        assert_eq!(error.source_document_id, checked.source_document_id);
        let before = &source[..error.span.start];
        assert_eq!(
            error.span.line,
            before.chars().filter(|c| *c == '\n').count() + 1
        );
        assert_eq!(
            error.span.column,
            before.rsplit('\n').next().unwrap().chars().count() + 1
        );
        assert_eq!(
            error.span.end_column - error.span.column,
            expected.chars().count()
        );
    }
    let mut foreign = parse_syntax_document(PHONETIC);
    let checked = check_syntax_document(&foreign, &catalogs.0).unwrap();
    foreign = parse_syntax_document(&format!("# another Source\n{PHONETIC}"));
    assert!(validate_source(&foreign, &checked).is_err());
}

#[test]
fn source_refuses_competing_phonemes_and_equal_spelling_across_types() {
    let catalogs = catalogs();
    let ambiguous = PHONEMIC
        .replace("ˈt͡ʃaː", "t͡ʃ")
        .replace("notation: \"aː\"", "notation: \"t͡ʃ\"")
        .replace("[\"unit/a\", \"unit/length\"]", "[\"unit/ch\"]");
    let syntax = parse_syntax_document(&ambiguous);
    let checked = check_syntax_document(&syntax, &catalogs.0).unwrap();
    let error = validate_source(&syntax, &checked).unwrap_err();
    assert!(matches!(
        error.cause.refusal,
        IpaConstructorRefusal::Inventory(
            conduit_speech::ipa_inventory::IpaInventoryRefusal::AmbiguousPhoneme
        )
    ));
    assert_eq!(&ambiguous[error.span.start..error.span.end], "t͡ʃ");
    let phone = PHONETIC
        .replace("speech/phonetic-from-ipa", "speech/phone-from-ipa")
        .replace("SpeechPhoneticTranscription", "SpeechPhoneNotation")
        .replace("ˈt͡ʃãː.n̩", "t͡ʃ");
    let phoneme = PHONEMIC
        .replace("speech/phonemic-from-ipa", "speech/phoneme-from-ipa")
        .replace("SpeechPhonemicTranscription", "SpeechPhonemeNotation")
        .replace("ˈt͡ʃaː", "t͡ʃ");
    for (source, original_type, wrong_type) in [
        (
            phone.as_str(),
            "SpeechPhoneNotation",
            "SpeechPhonemeNotation",
        ),
        (
            phoneme.as_str(),
            "SpeechPhonemeNotation",
            "SpeechPhoneNotation",
        ),
        (
            PHONETIC,
            "SpeechPhoneticTranscription",
            "SpeechPhonemicTranscription",
        ),
        (
            PHONEMIC,
            "SpeechPhonemicTranscription",
            "SpeechPhoneticTranscription",
        ),
    ] {
        let syntax = parse_syntax_document(&source.replace(original_type, wrong_type));
        let checked = check_syntax_document(&syntax, &catalogs.0).unwrap();
        assert!(expand_canonical_plot_for_authoring(
            &checked,
            &syntax.plots[0].name.text,
            &catalogs.1,
        )
        .is_err());
    }
}

#[test]
fn preparation_refuses_counterfeit_profiles_and_legacy_inventory_identity() {
    use conduit_core::StructuredConfigurationValue;
    let catalogs = catalogs();
    let syntax = parse_syntax_document(PHONEMIC);
    let checked = check_syntax_document(&syntax, &catalogs.0).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, &syntax.plots[0].name.text, &catalogs.1)
            .unwrap();
    let configuration = &authored.expanded.gears[0].configuration;
    assert!(prepare_configuration(IpaConstructor::Phonemic, configuration).is_ok());
    for index in 0..configuration.len() {
        let mut counterfeit = configuration.clone();
        let ConfigurationValue::Structured(original) = &configuration[index].value else {
            panic!("typed argument")
        };
        counterfeit[index].value = ConfigurationValue::Structured(
            StructuredConfigurationValue::new(
                "structured-info/profile-foreign@1".into(),
                original.canonical_value().to_vec(),
            )
            .unwrap(),
        );
        assert!(prepare_configuration(IpaConstructor::Phonemic, &counterfeit).is_err());
    }
    let mut legacy = configuration.clone();
    let entry = legacy
        .iter_mut()
        .find(|entry| entry.key == "inventory")
        .unwrap();
    let ConfigurationValue::Structured(original) = &entry.value else {
        panic!("inventory")
    };
    let mut bytes = original.canonical_value().to_vec();
    let schema = b"type/SpeechInventory@";
    let start = bytes
        .windows(schema.len())
        .position(|part| part == schema)
        .unwrap()
        + schema.len();
    // Exact shape-only root identity from the pinned #5327 generator. Keep all
    // selected payload bytes and its declared profile to test root substitution.
    bytes[start..start + 64]
        .copy_from_slice(b"18be04a2cf4dee09e520c22f45fcea3ea4bbc5f4b5e4f9d3859305ab5eb0c656");
    entry.value = ConfigurationValue::Structured(
        StructuredConfigurationValue::new(original.profile().clone(), bytes).unwrap(),
    );
    assert!(prepare_configuration(IpaConstructor::Phonemic, &legacy).is_err());
}

#[test]
fn source_round_trip_preserves_escaped_provenance_and_explicit_nasal_spelling() {
    let catalogs = catalogs();
    for nasal in ["ã", "ã"] {
        let body = format!("ˈt͡ʃ{nasal}ː.n̩");
        let source = PHONETIC.replace("ˈt͡ʃãː.n̩", &body).replace(
            "quoted transcription fixture",
            r#"reviewed \"quote\" \\ 雪"#,
        );
        let value = prepared(&source, IpaConstructor::Phonetic, &catalogs);
        let transcription = SpeechPhoneticTranscription::decode(value.bytes()).unwrap();
        assert_eq!(transcription.original(), &body);
        assert_eq!(
            transcription.provenance().method(),
            "reviewed \"quote\" \\ 雪"
        );
        assert_eq!(
            SpeechPhoneticTranscription::decode(&transcription.encode().unwrap()).unwrap(),
            transcription,
        );
    }
}
