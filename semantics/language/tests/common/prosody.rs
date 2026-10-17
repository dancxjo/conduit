//! Supplied exact UD fixtures and authored profile data, never parser accuracy.
use conduit_language::{discourse::*, lexical::*, *};
use conduit_plot::rust_binding::BoundedSequence;
pub fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "reviewed-prosody-fixture".into(),
        "fixture/1".into(),
    )
    .unwrap()
}
pub fn choice(
    boundary: LanguageProsodyBoundary,
    prominence: LanguageProsodyProminence,
    pitch: LanguageProsodyPitch,
) -> LanguageProsodyChoice {
    LanguageProsodyChoice::new(boundary, pitch, prominence).unwrap()
}
pub struct Fixture {
    pub lexical: PreparedLexicalTape,
    pub discourse: PreparedVocativeFact,
    pub profile: LanguageProsodyProfile,
}
pub fn fixture(text: &str, dependent: u64, head: u64) -> Fixture {
    let language = LanguageId::new("language/en".into()).unwrap();
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("text".into()).unwrap(),
            language.clone(),
            LanguageTextRevisionId::new("r1".into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap();
    let entry = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
            "Travis".into(),
            BoundedSequence::new(),
            LanguageLexicalPos::ProperNoun,
        )
        .unwrap()])
        .unwrap(),
        "Travis".into(),
    )
    .unwrap();
    let lexprofile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([entry]).unwrap(),
        "lexical/fixture".into(),
        language.clone(),
        provenance(),
    )
    .unwrap();
    let lexical = prepare_lexical_tape(&source, &lexprofile, None).unwrap();
    let analysis = LanguageAnalysisRevisionId::new("a1".into()).unwrap();
    let token = |ordinal| {
        LinguisticTokenIdentity::new(
            ordinal,
            source.material().identity().clone(),
            source.material().revision().clone(),
        )
        .unwrap()
    };
    let arc = LanguageDependencyArc::new(
        LanguageAnalysisTokenRef::new(analysis.clone(), token(dependent)).unwrap(),
        LanguageDependencyHead::token(analysis.clone(), token(head)).unwrap(),
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::Vocative, None)
            .unwrap(),
    )
    .unwrap();
    let discourse = prepare_vocative_fact(
        "fact/1".into(),
        &source,
        &analysis,
        &arc,
        provenance(),
        lexical.tape().tokens().len() as u64,
    )
    .unwrap();
    let profile = LanguageProsodyProfile::new(
        choice(
            LanguageProsodyBoundary::None,
            LanguageProsodyProminence::Neutral,
            LanguageProsodyPitch::Level,
        ),
        "prosody/fixture".into(),
        language,
        provenance(),
        choice(
            LanguageProsodyBoundary::MinorPhrase,
            LanguageProsodyProminence::Prominent,
            LanguageProsodyPitch::Level,
        ),
    )
    .unwrap();
    Fixture {
        lexical,
        discourse,
        profile,
    }
}
