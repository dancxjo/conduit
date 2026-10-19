//! Refusals reuse the positive fixture's actual current committed authority.
//! No extra parser execution, forged opaque receipt or unchecked current flag.
use super::lexical;
use conduit_language::{
    analysis_session::CurrentCommittedDependency,
    lexical::{prepare_lexical_tape, PreparedLexicalTape},
    pronunciation_selection::prepare_pronunciation_selection,
    *,
};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    ipa_inventory::PreparedIpaInventory,
    record_pronunciation::PreparedRecordPronunciationProfile,
    record_pronunciation_commitment::{
        CommittedRecordPronunciation, CommittedRecordPronunciationRefusal as Refusal,
    },
};

pub fn check(
    profile: &PreparedRecordPronunciationProfile,
    inventory: &PreparedIpaInventory<'_>,
    tape: &PreparedLexicalTape,
    authority: &CurrentCommittedDependency<'_, '_>,
) {
    let original = tape.tape();
    let arc = authority.receipt().arc();
    let analysis = arc.dependent().revision();
    // Same exact source and lexical tokens, but a different full profile.
    let changed_profile = LanguageLexicalProfile::new(
        original.profile().entries().clone(),
        "refusal/foreign-lexical-profile".into(),
        original.profile().language().clone(),
        original.profile().provenance().clone(),
    )
    .unwrap();
    let foreign = prepare_lexical_tape(original.source(), &changed_profile, None).unwrap();
    assert_eq!(foreign.tape().source(), original.source());
    assert_eq!(foreign.tape().tokens(), original.tokens());
    let selection =
        prepare_pronunciation_selection(&foreign, 1, analysis, arc, &lexical::profile()).unwrap();
    assert!(matches!(
        CommittedRecordPronunciation::prepare(profile, &selection, inventory, authority),
        Err(Refusal::LexicalProfile)
    ));

    // Same source, profile identity, token occurrence identity and arc. Alter
    // only the governor's candidate feature; comparing IDs alone would accept.
    let entries = original.profile().entries().iter().map(|entry| {
        if entry.surface() != "record" {
            return entry.clone();
        }
        let candidates = entry.candidates().iter().map(|candidate| {
            let mut features = candidate.morphology().as_slice().to_vec();
            features.push(
                LinguisticTokenFeature::new("fixture-custody".into(), "changed-governor".into())
                    .unwrap(),
            );
            LanguageLexicalCandidate::new(
                candidate.lemma().clone(),
                BoundedSequence::try_from_iter(features).unwrap(),
                *candidate.pos(),
            )
            .unwrap()
        });
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter(candidates).unwrap(),
            entry.surface().clone(),
        )
        .unwrap()
    });
    let changed_candidates = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).unwrap(),
        original.profile().identity().clone(),
        original.profile().language().clone(),
        original.profile().provenance().clone(),
    )
    .unwrap();
    let foreign = prepare_lexical_tape(original.source(), &changed_candidates, None).unwrap();
    assert_eq!(foreign.tape().source(), original.source());
    assert_eq!(
        foreign.tape().profile().identity(),
        original.profile().identity()
    );
    assert_eq!(
        foreign.tape().tokens()[1].identity(),
        original.tokens()[1].identity()
    );
    assert_ne!(foreign.tape().tokens()[1], original.tokens()[1]);
    let selection =
        prepare_pronunciation_selection(&foreign, 1, analysis, arc, &lexical::profile()).unwrap();
    // The full lexical profile gate catches this before the token gate; an
    // equal-profile different token cannot be produced by the opaque current
    // deterministic lexical preparation entrance without changing source.
    assert!(matches!(
        CommittedRecordPronunciation::prepare(profile, &selection, inventory, authority),
        Err(Refusal::LexicalProfile)
    ));

    let other_relation = if *arc.relation().base() == LanguageUniversalDependencyRelation::Det {
        LanguageUniversalDependencyRelation::Nsubj
    } else {
        LanguageUniversalDependencyRelation::Det
    };
    let changed_arc = LanguageDependencyArc::new(
        arc.dependent().clone(),
        arc.governor().clone(),
        LanguageDependencyRelation::new(other_relation, None).unwrap(),
    )
    .unwrap();
    let selection =
        prepare_pronunciation_selection(tape, 1, analysis, &changed_arc, &lexical::profile())
            .unwrap();
    assert!(matches!(
        CommittedRecordPronunciation::prepare(profile, &selection, inventory, authority),
        Err(Refusal::Arc)
    ));

    let foreign_analysis =
        LanguageAnalysisRevisionId::new("refusal/foreign-analysis".into()).unwrap();
    let LanguageDependencyHead::Token(head) = arc.governor() else {
        panic!("record governor")
    };
    let changed_analysis = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(foreign_analysis.clone(), arc.dependent().token().clone())
            .unwrap(),
        LanguageDependencyHead::token(foreign_analysis.clone(), head.token().clone()).unwrap(),
        arc.relation().clone(),
    )
    .unwrap();
    let selection = prepare_pronunciation_selection(
        tape,
        1,
        &foreign_analysis,
        &changed_analysis,
        &lexical::profile(),
    )
    .unwrap();
    assert!(matches!(
        CommittedRecordPronunciation::prepare(profile, &selection, inventory, authority),
        Err(Refusal::Analysis)
    ));
}
