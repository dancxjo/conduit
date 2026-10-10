#![cfg(feature = "semantic-bindings")]
use conduit_core::ConfigurationEntry;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    prepare_static_constructor, ProfileCatalog, StartupCatalog, StaticConstructorRefusal,
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

fn configuration(
    source: &str,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Vec<ConfigurationEntry> {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, startup).unwrap();
    expand_canonical_plot_for_authoring(&checked, &syntax.plots[0].name.text, profile)
        .unwrap()
        .expanded
        .gears[0]
        .configuration
        .clone()
}

#[test]
fn generic_static_admission_executes_each_canonical_ipa_constructor() {
    let (startup, profile) = catalogs();
    for (constructor, source) in [
        (IpaConstructor::Phonetic, PHONETIC.into()),
        (IpaConstructor::Phonemic, PHONEMIC.into()),
        (
            IpaConstructor::Phone,
            PHONETIC
                .replace("speech/phonetic-from-ipa", "speech/phone-from-ipa")
                .replace("SpeechPhoneticTranscription", "SpeechPhoneNotation")
                .replace("ˈt͡ʃãː.n̩", "t͡ʃ"),
        ),
        (
            IpaConstructor::Phoneme,
            PHONEMIC
                .replace("speech/phonemic-from-ipa", "speech/phoneme-from-ipa")
                .replace("SpeechPhonemicTranscription", "SpeechPhonemeNotation")
                .replace("ˈt͡ʃaː", "t͡ʃ"),
        ),
    ] {
        let configuration = configuration(&source, &startup, &profile);
        let ordinary = prepare_configuration(constructor, &configuration).unwrap();
        let prepared =
            prepare_static_constructor(&constructor, &startup, &profile, &configuration).unwrap();
        assert_eq!(prepared.constructor_kind().as_str(), constructor.kind());
        assert_eq!(prepared.constructor_revision().as_str(), REVISION);
        assert_eq!(prepared.configuration(), configuration);
        assert_eq!(prepared.value().value_type(), &constructor.output_type());
        assert_eq!(
            prepared.value().canonical_bytes().unwrap(),
            ordinary.bytes()
        );
        assert!(prepare_static_constructor(
            &constructor,
            &startup,
            &ProfileCatalog::new(),
            &configuration
        )
        .is_err());
    }
}

#[test]
fn one_segment_transcription_stays_the_declared_transcription_type() {
    let (startup, profile) = catalogs();
    for (constructor, source) in [
        (IpaConstructor::Phonetic, PHONETIC.replace("ˈt͡ʃãː.n̩", "t͡ʃ")),
        (IpaConstructor::Phonemic, PHONEMIC.replace("ˈt͡ʃaː", "t͡ʃ")),
    ] {
        let configuration = configuration(&source, &startup, &profile);
        let prepared =
            prepare_static_constructor(&constructor, &startup, &profile, &configuration).unwrap();
        let bytes = prepared.value().canonical_bytes().unwrap();
        match constructor {
            IpaConstructor::Phonetic => {
                assert_eq!(
                    SpeechPhoneticTranscription::decode(&bytes)
                        .unwrap()
                        .original(),
                    "t͡ʃ"
                );
                assert!(SpeechPhoneNotation::decode(&bytes).is_err());
            }
            IpaConstructor::Phonemic => {
                assert_eq!(
                    SpeechPhonemicTranscription::decode(&bytes)
                        .unwrap()
                        .original(),
                    "t͡ʃ"
                );
                assert!(SpeechPhonemeNotation::decode(&bytes).is_err());
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn request_shape_never_substitutes_for_domain_grammar_or_inventory_admission() {
    let (startup, profile) = catalogs();
    for (constructor, source) in [
        (
            IpaConstructor::Phonetic,
            PHONETIC.replace("ˈt͡ʃãː.n̩", r"t͡ʃ\n"),
        ),
        (IpaConstructor::Phonemic, PHONEMIC.replace("ˈt͡ʃaː", "z")),
        (
            IpaConstructor::Phonemic,
            PHONEMIC.replacen(
                "inventory_id: \"inventory/test\"",
                "inventory_id: \"inventory/foreign\"",
                1,
            ),
        ),
    ] {
        let configuration = configuration(&source, &startup, &profile);
        assert!(
            matches!(
                prepare_static_constructor(&constructor, &startup, &profile, &configuration),
                Err(StaticConstructorRefusal::Owner(_))
            ),
            "{source}"
        );
    }
    let mut configuration = configuration(PHONEMIC, &startup, &profile);
    let duplicate = configuration[0].clone();
    configuration.push(duplicate);
    assert!(matches!(
        prepare_static_constructor(
            &IpaConstructor::Phonemic,
            &startup,
            &profile,
            &configuration
        ),
        Err(StaticConstructorRefusal::Configuration)
    ));
    configuration.remove(0);
    configuration[0].key = "unknown".into();
    assert!(matches!(
        prepare_static_constructor(
            &IpaConstructor::Phonemic,
            &startup,
            &profile,
            &configuration
        ),
        Err(StaticConstructorRefusal::Configuration)
    ));
}
