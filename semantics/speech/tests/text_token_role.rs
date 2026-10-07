#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/vocative_intent.rs"]
#[allow(dead_code)]
mod intent;
#[path = "common/vocative_language.rs"]
#[allow(dead_code)]
mod language;
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{lexical_pronunciation::*, semantic::*, text_token_role::*};
fn fixture(
    text: &str,
) -> (
    PreparedLexicalTape,
    LanguageAnalysisRevisionId,
    Vec<LanguageDependencyArc>,
) {
    let provenance = language::language_provenance();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("punct/text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new("punct/r0".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        None,
    )
    .unwrap();
    let rows = [
        ("Hello", "hello", LanguageLexicalPos::Interjection),
        ("Travis", "Travis", LanguageLexicalPos::ProperNoun),
        (",", ",", LanguageLexicalPos::Punctuation),
        (".", ".", LanguageLexicalPos::Punctuation),
    ];
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(rows.map(|(surface, lemma, pos)| {
            LanguageLexicalEntry::new(
                BoundedSequence::try_from_iter([language::candidate(lemma, pos)]).unwrap(),
                surface.into(),
            )
            .unwrap()
        }))
        .unwrap(),
        "punct/profile".into(),
        source.material().language().clone(),
        provenance,
    )
    .unwrap();
    let lexical = prepare_lexical_tape(&source, &profile, None).unwrap();
    let analysis = LanguageAnalysisRevisionId::new("punct/analysis".into()).unwrap();
    let arcs = lexical
        .tape()
        .tokens()
        .iter()
        .enumerate()
        .map(|(ordinal, token)| {
            let relation = if ordinal == 0 {
                LanguageUniversalDependencyRelation::Root
            } else if ordinal == 2 {
                LanguageUniversalDependencyRelation::Vocative
            } else {
                LanguageUniversalDependencyRelation::Punct
            };
            LanguageDependencyArc::new(
                LanguageAnalysisTokenRef::new(analysis.clone(), token.identity().clone()).unwrap(),
                if ordinal == 0 {
                    LanguageDependencyHead::Root
                } else {
                    LanguageDependencyHead::token(
                        analysis.clone(),
                        lexical.tape().tokens()[0].identity().clone(),
                    )
                    .unwrap()
                },
                LanguageDependencyRelation::new(relation, None).unwrap(),
            )
            .unwrap()
        })
        .collect();
    (lexical, analysis, arcs)
}
#[test]
fn source_roles_retain_punctuation_and_refuse_foreign_or_inconsistent_basis() {
    let (lexical, analysis, arcs) = fixture("Hello, Travis.");
    let expected = [
        SpeechTextTokenRole::Spoken,
        SpeechTextTokenRole::Nonspoken,
        SpeechTextTokenRole::Spoken,
        SpeechTextTokenRole::Nonspoken,
    ];
    for (ordinal, role) in expected.iter().enumerate() {
        let admitted =
            prepare_text_token_role(&lexical, ordinal, &analysis, &arcs[ordinal], 0).unwrap();
        assert_eq!(admitted.result().role(), role);
        assert_eq!(admitted.request().source(), lexical.tape().source());
        assert_eq!(admitted.request().basis(), &arcs[ordinal]);
        assert_eq!(
            SpeechTextTokenRoleRequest::decode(&admitted.request().clone().encode().unwrap())
                .unwrap(),
            *admitted.request()
        );
    }
    let foreign = LanguageAnalysisRevisionId::new("foreign/analysis".into()).unwrap();
    assert!(prepare_text_token_role(&lexical, 1, &foreign, &arcs[1], 0).is_err());
    assert!(prepare_text_token_role(&lexical, 1, &analysis, &arcs[2], 0).is_err());
    assert!(prepare_text_token_role(&lexical, 1, &analysis, &arcs[1], 4).is_err());
    let contradictory = LanguageDependencyArc::new(
        arcs[2].dependent().clone(),
        arcs[2].governor().clone(),
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Punct, None).unwrap(),
    )
    .unwrap();
    let Err(TextTokenRoleRefusal::Inconsistent(rejected)) =
        prepare_text_token_role(&lexical, 2, &analysis, &contradictory, 0)
    else {
        panic!("Source refusal")
    };
    assert_eq!(rejected.request().basis(), &contradictory);
    assert_eq!(rejected.result().role(), &SpeechTextTokenRole::Refused);
    let mut catalog = conduit_plot::StartupCatalog::new();
    for (name, ty) in text_token_role_types() {
        catalog.insert_structured_type(name, ty).unwrap();
    }
}
#[test]
fn rich_source_prosody_overrides_comma_pause_without_erasing_material() {
    // This graph is explicitly supplied; no learned parser accuracy is claimed.
    let (lexical, analysis, arcs) = fixture("Hello, Travis.");
    let case = language::admitted_graph_with_choices(lexical, analysis, arcs, 2, &[0; 4]);
    assert_eq!(case.spoken_ordinals, vec![0, 2]);
    assert_eq!(case.participation.len(), 4);
    let pronunciation = case
        .selections
        .iter()
        .map(|s| prepare_pronunciation(s, &case.phones).unwrap())
        .collect::<Vec<_>>();
    let composite = intent::compose(&case, &pronunciation);
    assert_eq!(composite.words.len(), 2);
    assert_eq!(composite.segments.len(), 10);
    let linguistic = composite.linguistic(&case);
    assert_eq!(
        linguistic.len(),
        10,
        "all phones retain original lexical ordinal prosody"
    );
    let boundaries = composite
        .source
        .events()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, event)| {
            matches!(event, SpeechUtteranceIntentEvent::Boundary(_)).then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        boundaries,
        vec![10],
        "only the Source-selected post-vocative boundary"
    );
    let mut storage = [conduit_speech::VoiceEvent::boundary(conduit_speech::VoiceBoundary::word);
        conduit_speech::MAXIMUM_EVENTS];
    let fallback =
        conduit_speech::pronounce(case.lexical.tape().source().material().text(), &mut storage)
            .unwrap();
    assert!(fallback.events().iter().any(|event| matches!(
        event,
        conduit_speech::VoiceEvent::boundary(conduit_speech::VoiceBoundary::phrase)
    )));
    let (changed, _, _) = fixture("Hello. Travis.");
    assert_ne!(
        changed.tape().source().material(),
        case.lexical.tape().source().material()
    );
    assert!(
        prepare_text_revision_lineage(case.lexical.tape().source(), changed.tape().source())
            .is_err(),
        "changed punctuation under same source IDs cannot reinterpret or commit old material"
    );
    assert_eq!(
        case.participation[1].request().source().material().text(),
        "Hello, Travis."
    );
}
