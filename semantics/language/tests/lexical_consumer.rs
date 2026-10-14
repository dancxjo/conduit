use conduit_language::{lexical_consumer::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
#[path = "common/pronunciation_fixture.rs"]
mod fixture;
fn variety(name: &str) -> LanguageVariety {
    LanguageVariety::new(
        VarietyId::new(name.into()).unwrap(),
        LanguageId::new("language/en".into()).unwrap(),
    )
    .unwrap()
}
fn inventory(features: bool) -> LanguageLexicalConsumerInventory {
    let supplied = if features {
        vec![
            LinguisticTokenFeature::new("Number".into(), "Singular".into()).unwrap(),
            LinguisticTokenFeature::new("Tense".into(), "Present".into()).unwrap(),
        ]
    } else {
        vec![]
    };
    LanguageLexicalConsumerInventory::new(
        BoundedSequence::try_from_iter(supplied).unwrap(),
        "consumer/1".into(),
        LanguageId::new("language/en".into()).unwrap(),
        fixture::provenance(),
        BoundedSequence::try_from_iter([variety("supported")]).unwrap(),
    )
    .unwrap()
}
#[test]
fn ambiguous_analyses_keep_original_occurrence_and_supported_morphology() {
    for (text, relation, pos) in [
        (
            "a record",
            LanguageUniversalDependencyRelation::Det,
            LanguageLexicalPos::Noun,
        ),
        (
            "we record",
            LanguageUniversalDependencyRelation::Nsubj,
            LanguageLexicalPos::Verb,
        ),
    ] {
        let lexical = fixture::lexical(text);
        let arc = fixture::arc(&lexical, relation);
        let original_inventory = inventory(true);
        let admitted =
            LanguageLexicalConsumerInventory::decode(&original_inventory.encode().unwrap())
                .unwrap();
        let supplied_variety = variety("supported");
        let prepared = PreparedLexicalConsumer::prepare(
            &lexical,
            1,
            &fixture::analysis(),
            &arc,
            &fixture::profile(),
            &admitted,
            &supplied_variety,
        )
        .unwrap();
        assert!(core::ptr::eq(prepared.lexical(), &lexical));
        assert_eq!(
            prepared.selection().request().token().identity(),
            lexical.tape().tokens()[1].identity()
        );
        assert_eq!(prepared.selection().request().token().candidates().len(), 2);
        assert_eq!(prepared.selection().candidate().pos(), &pos);
        assert_eq!(prepared.features().len(), 2);
        assert_eq!(prepared.selection().candidate().morphology().len(), 1);
        assert_eq!(prepared.variety(), &supplied_variety);
        assert_eq!(prepared.basis().source(), lexical.tape().source());
        let bytes = prepared.selection().request().clone().encode().unwrap();
        assert_eq!(
            LanguagePronunciationSelectionRequest::decode(&bytes).unwrap(),
            *prepared.selection().request()
        );
    }
}
#[test]
fn unsupported_features_varieties_and_foreign_occurrences_refuse() {
    let lexical = fixture::lexical("a record");
    let arc = fixture::arc(&lexical, LanguageUniversalDependencyRelation::Det);
    let supported = inventory(true);
    let unsupported = inventory(false);
    let known = variety("supported");
    assert!(matches!(
        PreparedLexicalConsumer::prepare(
            &lexical,
            1,
            &fixture::analysis(),
            &arc,
            &fixture::profile(),
            &unsupported,
            &known
        ),
        Err(LexicalConsumerRefusal::Feature)
    ));
    assert!(matches!(
        PreparedLexicalConsumer::prepare(
            &lexical,
            1,
            &fixture::analysis(),
            &arc,
            &fixture::profile(),
            &supported,
            &variety("foreign")
        ),
        Err(LexicalConsumerRefusal::Variety)
    ));
    let foreign = fixture::lexical("we record");
    assert!(PreparedLexicalConsumer::prepare(
        &foreign,
        1,
        &fixture::analysis(),
        &arc,
        &fixture::profile(),
        &supported,
        &known
    )
    .is_err());
    let overbudget = BoundedSequence::<LanguageLexicalCandidate, 4>::try_from_iter(vec![
            fixture::candidate(
                LanguageLexicalPos::Noun
            );
            5
        ]);
    assert!(overbudget.is_err());
}

#[test]
fn same_text_identity_with_foreign_text_revision_cannot_consume_old_arc() {
    let lexical = fixture::lexical("a record");
    let arc = fixture::arc(&lexical, LanguageUniversalDependencyRelation::Det);
    let old = lexical.tape().source().material();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            old.identity().clone(),
            old.language().clone(),
            LanguageTextRevisionId::new("foreign-r1".into()).unwrap(),
            old.text().clone(),
        )
        .unwrap(),
        None,
        fixture::provenance(),
        0,
        None,
    )
    .unwrap();
    let foreign =
        conduit_language::lexical::prepare_lexical_tape(&source, lexical.tape().profile(), None)
            .unwrap();
    assert_eq!(
        foreign.tape().source().material().identity(),
        old.identity()
    );
    assert_ne!(
        foreign.tape().source().material().revision(),
        old.revision()
    );
    assert!(PreparedLexicalConsumer::prepare(
        &foreign,
        1,
        &fixture::analysis(),
        &arc,
        &fixture::profile(),
        &inventory(true),
        &variety("supported")
    )
    .is_err());
}
