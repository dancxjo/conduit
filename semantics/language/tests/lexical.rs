use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::BoundedSequence;
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule("fixture/lexical".into(), "profile/1".into())
        .unwrap()
}
fn revision(
    text: &str,
    sequence: u64,
    previous: Option<&LanguageTextRevision>,
    stable: Option<u32>,
    finality: LanguageTextFinality,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new("text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(format!("r/{sequence}")).unwrap(),
            text.into(),
        )
        .unwrap(),
        previous.map(|old| {
            LanguageTextPriorRevision::new(old.material().revision().clone(), *old.sequence())
                .unwrap()
        }),
        provenance(),
        sequence,
        stable,
    )
    .unwrap()
}
fn profile() -> LanguageLexicalProfile {
    let candidate = |lemma: &str, pos| {
        LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos).unwrap()
    };
    let entries = BoundedSequence::try_from_iter([
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([candidate("answer", LanguageLexicalPos::Noun)])
                .unwrap(),
            "answer".into(),
        )
        .unwrap(),
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                candidate("record", LanguageLexicalPos::Noun),
                candidate("record", LanguageLexicalPos::Verb),
            ])
            .unwrap(),
            "record".into(),
        )
        .unwrap(),
    ])
    .unwrap();
    LanguageLexicalProfile::new(
        entries,
        "lexical/fixture".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}
#[test]
fn words_are_whole_and_lexical_ambiguity_is_profile_data() {
    let source = revision("answer record", 0, None, None, LanguageTextFinality::Final);
    let prepared = prepare_lexical_tape(&source, &profile(), None).unwrap();
    let tokens = prepared.tape().tokens();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].surface(), "answer");
    assert_eq!(tokens[1].candidates().len(), 2);
    assert!(matches!(
        tokens[1].candidates()[0].pos(),
        LanguageLexicalPos::Noun
    ));
    assert!(matches!(
        tokens[1].candidates()[1].pos(),
        LanguageLexicalPos::Verb
    ));
}
#[test]
fn stable_complete_occurrences_correspond_across_exact_revisions() {
    let profile = profile();
    let first = revision("answer re", 0, None, Some(7), LanguageTextFinality::Partial);
    let old = prepare_lexical_tape(&first, &profile, None).unwrap();
    assert!(matches!(
        old.tape().tokens()[1].completeness(),
        LanguageLexicalCompleteness::TrailingPartial
    ));
    assert!(old.tape().tokens()[1].candidates().is_empty());
    let second = revision(
        "answer record ",
        1,
        Some(&first),
        Some(7),
        LanguageTextFinality::Partial,
    );
    let new = prepare_lexical_tape(&second, &profile, Some(&old)).unwrap();
    assert_eq!(
        new.tape().tokens()[0].prior_occurrence(),
        &Some(old.tape().tokens()[0].identity().clone())
    );
    assert_ne!(
        new.tape().tokens()[0].identity(),
        old.tape().tokens()[0].identity()
    );
    assert_eq!(new.tape().tokens()[1].prior_occurrence(), &None);
    assert_eq!(new.tape().tokens()[1].candidates().len(), 2);
    assert!(matches!(
        prepare_lexical_tape(&second, &profile, None),
        Err(LexicalRefusal::Prior)
    ));
}
#[test]
fn scalar_spans_bounds_and_stale_source_are_checked() {
    let profile = profile();
    let first = revision("猫 answer", 0, None, None, LanguageTextFinality::Final);
    let tape = prepare_lexical_tape(&first, &profile, None).unwrap();
    assert_eq!(*tape.tape().tokens()[1].span().start(), 2);
    assert_eq!(*tape.tape().tokens()[1].span().end(), 8);
    assert!(matches!(
        prepare_lexical_tape(&first, &profile, Some(&tape)),
        Err(LexicalRefusal::Revision(_))
    ));
    let long = revision(&"a".repeat(257), 0, None, None, LanguageTextFinality::Final);
    assert!(matches!(
        prepare_lexical_tape(&long, &profile, None),
        Err(LexicalRefusal::TokenBytes)
    ));
    let many = revision(&"!".repeat(129), 0, None, None, LanguageTextFinality::Final);
    assert!(matches!(
        prepare_lexical_tape(&many, &profile, None),
        Err(LexicalRefusal::TokenBound)
    ));
}

#[test]
fn foreign_source_profiles_and_duplicate_data_refuse() {
    let profile = profile();
    let source = revision("answer ", 0, None, Some(7), LanguageTextFinality::Partial);
    let previous = prepare_lexical_tape(&source, &profile, None).unwrap();
    let foreign = LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            LanguageTextId::new("foreign".into()).unwrap(),
            source.material().language().clone(),
            LanguageTextRevisionId::new("foreign/r1".into()).unwrap(),
            "answer ".into(),
        )
        .unwrap(),
        Some(LanguageTextPriorRevision::new(source.material().revision().clone(), 0).unwrap()),
        provenance(),
        1,
        Some(7),
    )
    .unwrap();
    assert!(matches!(
        prepare_lexical_tape(&foreign, &profile, Some(&previous)),
        Err(LexicalRefusal::Revision(TextRevisionRefusal::Identity))
    ));
    let different = LanguageLexicalProfile::new(
        BoundedSequence::new(),
        "other".into(),
        profile.language().clone(),
        provenance(),
    )
    .unwrap();
    let next = revision(
        "answer record",
        1,
        Some(&source),
        Some(7),
        LanguageTextFinality::Final,
    );
    assert!(matches!(
        prepare_lexical_tape(&next, &different, Some(&previous)),
        Err(LexicalRefusal::Profile)
    ));
    let french = LanguageLexicalProfile::new(
        BoundedSequence::new(),
        "fr".into(),
        LanguageId::new("language/fr".into()).unwrap(),
        provenance(),
    )
    .unwrap();
    assert!(matches!(
        prepare_lexical_tape(&source, &french, None),
        Err(LexicalRefusal::Language)
    ));
    let duplicate = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([
            profile.entries()[0].clone(),
            profile.entries()[0].clone(),
        ])
        .unwrap(),
        "duplicate".into(),
        profile.language().clone(),
        provenance(),
    )
    .unwrap();
    assert!(matches!(
        prepare_lexical_tape(&source, &duplicate, None),
        Err(LexicalRefusal::DuplicateEntry)
    ));
    let unknown = prepare_lexical_tape(&source, &different, None).unwrap();
    assert!(unknown.tape().tokens()[0].candidates().is_empty());
}

#[test]
fn lexical_schemas_are_available_through_installed_catalog() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    for (name, _) in lexical_types() {
        let source = format!(
            "plot language/lexical-fixture (\n >> value: {name}\n result: {name} >>\n) = (.)"
        );
        conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(&source),
            &startup,
        )
        .unwrap();
    }
}
#[test]
fn unicode_runs_are_honest_unknown_units_not_segmented_language_claims() {
    let source = revision("猫は寝る", 0, None, None, LanguageTextFinality::Final);
    let tape = prepare_lexical_tape(&source, &profile(), None).unwrap();
    assert_eq!(tape.tape().tokens().len(), 1);
    assert_eq!(tape.tape().tokens()[0].surface(), "猫は寝る");
    assert!(tape.tape().tokens()[0].candidates().is_empty());
    assert_eq!(*tape.tape().tokens()[0].span().end(), 4);
}

#[test]
fn listed_punctuation_preserves_exact_mixed_occurrences_and_partial_abstention() {
    let original = profile();
    let mut entries = original.entries().iter().cloned().collect::<Vec<_>>();
    entries.push(LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
            ",".into(), BoundedSequence::new(), LanguageLexicalPos::Punctuation,
        ).unwrap()]).unwrap(), ",".into(),
    ).unwrap());
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(), "punctuation/fixture".into(),
        original.language().clone(), provenance(),
    ).unwrap();
    let first = revision("answer, re", 0, None, Some(8), LanguageTextFinality::Partial);
    let old = prepare_lexical_tape(&first, &profile, None).unwrap();
    let tokens = old.tape().tokens();
    assert_eq!(tokens.len(), 3);
    for (ordinal, token) in tokens.iter().enumerate() {
        assert_eq!(*token.identity().ordinal(), ordinal as u64);
        assert_eq!(token.span().text_revision(), first.material().revision());
    }
    assert_eq!((*tokens[1].span().start(), *tokens[1].span().end()), (6, 7));
    assert_eq!(tokens[1].candidates().len(), 1);
    assert!(tokens[2].candidates().is_empty());
    let second = revision("answer, record !", 1, Some(&first), Some(8), LanguageTextFinality::Final);
    let new = prepare_lexical_tape(&second, &profile, Some(&old)).unwrap();
    assert_eq!(new.tape().tokens()[1].prior_occurrence(), &Some(tokens[1].identity().clone()));
    assert_eq!(new.tape().tokens()[2].candidates().len(), 2);
    assert!(new.tape().tokens()[3].candidates().is_empty());
    assert_eq!(new.tape().tokens()[1].span().text_revision(), second.material().revision());
}
