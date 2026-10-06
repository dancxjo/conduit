//! Supplied Japanese segmentation and dependencies pressure-test the shared contracts.
//! This is architectural conformance, not a second parser or accuracy evidence.
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn material(revision: &str) -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("text/japanese-specimen".into()).unwrap(),
        LanguageId::new("language/japanese".into()).unwrap(),
        LanguageTextRevisionId::new(revision.into()).unwrap(),
        "猫が寝る。".into(),
    )
    .unwrap()
}
fn specimen(source: &LanguageText) -> LinguisticTokensFour {
    let token = |ordinal, surface: &str, start, end, particle| {
        LinguisticToken::new(
            if surface == "。" {
                LinguisticTokenCategory::punctuation()
            } else {
                LinguisticTokenCategory::word()
            },
            [
                if particle {
                    LinguisticTokenFeatureSlot::feature("PartOfSpeech".into(), "Particle".into())
                        .unwrap()
                } else {
                    LinguisticTokenFeatureSlot::unused()
                },
                LinguisticTokenFeatureSlot::unused(),
            ],
            LinguisticTokenIdentity::new(
                ordinal,
                source.identity().clone(),
                source.revision().clone(),
            )
            .unwrap(),
            LinguisticOptionalText::absent(),
            TextSpan::new(
                LinguisticOffsetBasis::unicode_scalar(),
                end,
                start,
                source.identity().clone(),
                source.revision().clone(),
            )
            .unwrap(),
            surface.into(),
        )
        .unwrap()
    };
    LinguisticTokensFour::new(
        LinguisticDerivationProvenance::deterministic_rule(
            "supplied/Japanese-segmentation".into(),
            "specimen@1".into(),
        )
        .unwrap(),
        [LinguisticSegment::new(
            "sentence/0".into(),
            LinguisticSegmentKind::sentence(),
            TextSpan::new(
                LinguisticOffsetBasis::unicode_scalar(),
                5,
                0,
                source.identity().clone(),
                source.revision().clone(),
            )
            .unwrap(),
        )
        .unwrap()],
        source.clone(),
        [
            token(0, "猫", 0, 1, false),
            token(1, "が", 1, 2, true),
            token(2, "寝る", 2, 4, false),
            token(3, "。", 4, 5, false),
        ],
    )
    .unwrap()
}

#[test]
fn non_latin_particle_and_word_order_use_shared_source_tokens_and_dependency_contracts() {
    let source = material("source/1");
    let tokens = specimen(&source);
    validate_linguistic_source(&tokens).unwrap();
    let decoded = LinguisticTokensFour::decode(&tokens.clone().encode().unwrap()).unwrap();
    assert_eq!(decoded, tokens);
    assert_eq!(decoded.source(), &source);
    let occurrence = language_source_occurrence(
        &source,
        tokens.tokens()[2].span(),
        LanguageTextSegmentKind::Word,
    )
    .unwrap();
    assert_eq!(
        (*occurrence.range().start(), *occurrence.range().end()),
        (2, 4)
    );
    assert_eq!(occurrence.revision_id(), source.revision());
    let reference = |index: usize| {
        LanguageAnalysisTokenRef::new(
            LanguageAnalysisRevisionId::new("analysis/7".into()).unwrap(),
            tokens.tokens()[index].identity().clone(),
        )
        .unwrap()
    };
    for (dependent, governor, relation) in [
        (0, 2, LanguageUniversalDependencyRelation::nsubj()),
        (1, 0, LanguageUniversalDependencyRelation::case()),
        (3, 2, LanguageUniversalDependencyRelation::punct()),
    ] {
        let head = reference(governor);
        let arc = LanguageDependencyArc::new(
            reference(dependent),
            LanguageDependencyHead::token(head.revision().clone(), head.token().clone()).unwrap(),
            LanguageDependencyRelation::new(relation, None).unwrap(),
        )
        .unwrap();
        assert_eq!(
            LanguageDependencyArc::decode(&arc.clone().encode().unwrap()).unwrap(),
            arc
        );
        assert_eq!(arc.dependent().token().text_revision(), source.revision());
        assert_ne!(arc.dependent().revision().get(), source.revision().get());
    }
}

#[test]
fn stale_source_revision_and_wrong_language_are_distinct_refusals() {
    let source = material("source/1");
    let tokens = specimen(&source);
    let revised = material("source/2");
    assert_eq!(
        language_source_occurrence(
            &revised,
            tokens.tokens()[0].span(),
            LanguageTextSegmentKind::Word
        ),
        Err(LinguisticRefusal::SourceRevision)
    );
    let request = LanguageRequest::new(
        LanguageId::new("language/english".into()).unwrap(),
        None,
        LanguageVarietyPolicy::LanguageSufficient,
    )
    .unwrap();
    assert_eq!(
        validate_linguistic_request(&request, &tokens),
        Err(LinguisticRefusal::SourceLanguage)
    );
    let domain = revision::DependencyRevisions {
        source_text: &revised,
        tokens: 4,
    };
    let delta = revision::DependencyDelta::Withdraw(tokens.tokens()[0].identity().clone());
    assert!(!conduit_core::revision::RevisionDomain::validate_delta(
        &domain,
        conduit_core::revision::RevisionDeltaRole::Withdrawal,
        &delta
    ));
}

#[test]
fn byte_basis_is_converted_exactly_and_a_split_utf8_scalar_refuses() {
    let source = material("source/1");
    let span = |start, end| {
        TextSpan::new(
            LinguisticOffsetBasis::utf8_byte(),
            end,
            start,
            source.identity().clone(),
            source.revision().clone(),
        )
        .unwrap()
    };
    let occurrence =
        language_source_occurrence(&source, &span(6, 12), LanguageTextSegmentKind::Word).unwrap();
    assert_eq!(
        (*occurrence.range().start(), *occurrence.range().end()),
        (2, 4)
    );
    assert_eq!(
        language_source_occurrence(&source, &span(7, 12), LanguageTextSegmentKind::Word),
        Err(LinguisticRefusal::SourceRange)
    );
}
