//! Explicit supplied canonical graphs. These fixtures prove admission and the
//! checked policy pipeline, never learned parser accuracy or person identity.
use conduit_language::{discourse::*, lexical::*, pronunciation_selection::*, prosody::*, *};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
pub struct Case {
    pub lexical: PreparedLexicalTape,
    pub analysis: LanguageAnalysisRevisionId,
    pub arcs: Vec<LanguageDependencyArc>,
    pub vocative: usize,
    pub rich: PreparedRichProsody,
    pub fallback: Vec<PreparedFallbackProsody>,
    pub selections: Vec<PreparedPronunciationSelection>,
    pub phones: SpeechPronunciationProfile,
    pub inventory: SpeechInventory,
    pub voice: SpeechFormantVoiceProfile,
    pub boundaries: SpeechFormantBoundaryProfile,
}
pub fn language_provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "supplied-canonical-graph/proof".into(),
        "reviewed/1".into(),
    )
    .unwrap()
}
pub fn speech_provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "supplied graph and reviewed pronunciation/proof".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
pub fn candidate(lemma: &str, pos: LanguageLexicalPos) -> LanguageLexicalCandidate {
    LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos).unwrap()
}
fn lexical_profile() -> LanguageLexicalProfile {
    let rows = [
        ("Hello", "hello", LanguageLexicalPos::Interjection),
        ("Travis", "Travis", LanguageLexicalPos::ProperNoun),
        ("friend", "friend", LanguageLexicalPos::Noun),
    ];
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(rows.map(|(surface, lemma, pos)| {
            LanguageLexicalEntry::new(
                BoundedSequence::try_from_iter([candidate(lemma, pos)]).unwrap(),
                surface.into(),
            )
            .unwrap()
        }))
        .unwrap(),
        "proof/lexical".into(),
        LanguageId::new("language/en".into()).unwrap(),
        language_provenance(),
    )
    .unwrap()
}
fn selection_profile() -> LanguagePronunciationSelectionProfile {
    let relation = |base| LanguageDependencyRelation::new(base, None).unwrap();
    LanguagePronunciationSelectionProfile::new(
        "proof/dependent-pos".into(),
        LanguageId::new("language/en".into()).unwrap(),
        language_provenance(),
        BoundedSequence::try_from_iter([
            LanguagePronunciationSelectionRule::new(
                LanguageLexicalPos::Interjection,
                relation(LanguageUniversalDependencyRelation::Root),
            )
            .unwrap(),
            LanguagePronunciationSelectionRule::new(
                LanguageLexicalPos::ProperNoun,
                relation(LanguageUniversalDependencyRelation::Vocative),
            )
            .unwrap(),
            LanguagePronunciationSelectionRule::new(
                LanguageLexicalPos::Noun,
                relation(LanguageUniversalDependencyRelation::Obj),
            )
            .unwrap(),
        ])
        .unwrap(),
        LanguagePronunciationArcTarget::Dependent,
    )
    .unwrap()
}
fn phones() -> SpeechPronunciationProfile {
    let phone = |id: &str, stress| {
        SpeechPronunciationPhone::new(PhoneId::new(id.into()).unwrap(), stress).unwrap()
    };
    let row = |lemma, pos, ids: &[(&str, SpeechStress)]| {
        SpeechPronunciationRow::new(
            candidate(lemma, pos),
            BoundedSequence::try_from_iter(ids.iter().map(|(id, stress)| phone(id, *stress)))
                .unwrap(),
        )
        .unwrap()
    };
    SpeechPronunciationProfile::new(
        "proof/phones".into(),
        LanguageId::new("language/en".into()).unwrap(),
        speech_provenance(),
        BoundedSequence::try_from_iter([
            row(
                "hello",
                LanguageLexicalPos::Interjection,
                &[
                    ("h", SpeechStress::Unstressed),
                    ("ax", SpeechStress::Unstressed),
                    ("l", SpeechStress::Unstressed),
                    ("ow", SpeechStress::Primary),
                ],
            ),
            row(
                "Travis",
                LanguageLexicalPos::ProperNoun,
                &[
                    ("t", SpeechStress::Unstressed),
                    ("r", SpeechStress::Unstressed),
                    ("ae", SpeechStress::Primary),
                    ("v", SpeechStress::Unstressed),
                    ("ih", SpeechStress::Unstressed),
                    ("s", SpeechStress::Unstressed),
                ],
            ),
            row(
                "friend",
                LanguageLexicalPos::Noun,
                &[
                    ("f", SpeechStress::Unstressed),
                    ("r", SpeechStress::Unstressed),
                    ("eh", SpeechStress::Primary),
                    ("n", SpeechStress::Unstressed),
                    ("d", SpeechStress::Unstressed),
                ],
            ),
        ])
        .unwrap(),
    )
    .unwrap()
}
pub fn case(position: usize, revision: &str, previous: Option<&Case>) -> Case {
    let (text, hello, vocative) = match position {
        0 => ("Travis Hello", 1, 0),
        1 => ("Hello Travis friend", 0, 1),
        2 => ("Hello friend Travis", 0, 2),
        _ => panic!("three reviewed positions"),
    };
    let source = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("proof/text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(revision.into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        previous.map(|old| {
            LanguageTextPriorRevision::new(
                old.lexical.tape().source().material().revision().clone(),
                *old.lexical.tape().source().sequence(),
            )
            .unwrap()
        }),
        language_provenance(),
        previous.map_or(0, |old| *old.lexical.tape().source().sequence() + 1),
        None,
    )
    .unwrap();
    let lexical = prepare_lexical_tape(
        &source,
        &lexical_profile(),
        previous.map(|old| &old.lexical),
    )
    .unwrap();
    let analysis = LanguageAnalysisRevisionId::new(format!("proof/analysis/{revision}")).unwrap();
    let reference = |ordinal: usize| {
        LanguageAnalysisTokenRef::new(
            analysis.clone(),
            lexical.tape().tokens()[ordinal].identity().clone(),
        )
        .unwrap()
    };
    let arcs = (0..lexical.tape().tokens().len())
        .map(|ordinal| {
            let (head, relation) = if ordinal == hello {
                (
                    LanguageDependencyHead::Root,
                    LanguageUniversalDependencyRelation::Root,
                )
            } else {
                (
                    LanguageDependencyHead::token(
                        analysis.clone(),
                        lexical.tape().tokens()[hello].identity().clone(),
                    )
                    .unwrap(),
                    if ordinal == vocative {
                        LanguageUniversalDependencyRelation::Vocative
                    } else {
                        LanguageUniversalDependencyRelation::Obj
                    },
                )
            };
            LanguageDependencyArc::new(
                reference(ordinal),
                head,
                LanguageDependencyRelation::new(relation, None).unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    admitted_graph(lexical, analysis, arcs, vocative)
}
/// Shared checked policy route. Graph acquisition is owned by the caller;
/// supplying a graph here is not evidence of learned parser accuracy.
pub fn admitted_graph(
    lexical: PreparedLexicalTape,
    analysis: LanguageAnalysisRevisionId,
    arcs: Vec<LanguageDependencyArc>,
    vocative: usize,
) -> Case {
    let source = lexical.tape().source().clone();
    let revision = source.material().revision().get();
    let discourse = prepare_vocative_fact(
        format!("proof/fact/{revision}"),
        &source,
        &analysis,
        &arcs[vocative],
        language_provenance(),
        lexical.tape().tokens().len() as u64,
    )
    .unwrap();
    let profile = LanguageProsodyProfile::new(
        LanguageProsodyChoice::new(
            LanguageProsodyBoundary::None,
            LanguageProsodyPitch::Level,
            LanguageProsodyProminence::Neutral,
        )
        .unwrap(),
        "proof/prosody".into(),
        source.material().language().clone(),
        language_provenance(),
        LanguageProsodyChoice::new(
            LanguageProsodyBoundary::MajorPhrase,
            LanguageProsodyPitch::Rising,
            LanguageProsodyProminence::Prominent,
        )
        .unwrap(),
    )
    .unwrap();
    let rich = prepare_rich_prosody(&lexical, vocative, discourse.fact(), &profile).unwrap();
    let fallback = (0..lexical.tape().tokens().len())
        .map(|ordinal| prepare_fallback_prosody(&lexical, ordinal, &profile).unwrap())
        .collect();
    let selections = arcs
        .iter()
        .enumerate()
        .map(|(ordinal, arc)| {
            prepare_pronunciation_selection(&lexical, ordinal, &analysis, arc, &selection_profile())
                .unwrap()
        })
        .collect();
    use conduit_speech::semantic::EnglishPhone as E;
    let rows = [
        ("h", "h", E::H),
        ("ax", "ə", E::Ax),
        ("l", "l", E::L),
        ("ow", "oʊ", E::Ow),
        ("t", "t", E::T),
        ("r", "ɹ", E::R),
        ("ae", "æ", E::Ae),
        ("v", "v", E::V),
        ("ih", "ɪ", E::Ih),
        ("s", "s", E::S),
        ("f", "f", E::F),
        ("eh", "ɛ", E::Eh),
        ("n", "n", E::N),
        ("d", "d", E::D),
    ];
    let definitions = rows
        .iter()
        .map(|(id, ipa, _)| {
            SpeechPhone::new(
                BoundedSequence::new(),
                SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
                PhoneId::new((*id).into()).unwrap(),
                (*ipa).into(),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let inventory_id = SpeechInventoryId::new("proof/inventory".into()).unwrap();
    let voice = SpeechFormantVoiceProfile::new(
        "proof/voice".into(),
        inventory_id.clone(),
        source.material().language().clone(),
        BoundedSequence::try_from_iter(definitions.iter().zip(rows).map(
            |(definition, (_, _, phone))| {
                SpeechFormantPhoneBinding::new(definition.clone(), phone).unwrap()
            },
        ))
        .unwrap(),
    )
    .unwrap();
    let inventory = SpeechInventory::new(
        inventory_id,
        source.material().language().clone(),
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(definitions).unwrap(),
    )
    .unwrap();
    Case {
        lexical,
        analysis,
        arcs,
        vocative,
        rich,
        fallback,
        selections,
        phones: phones(),
        inventory,
        voice,
        boundaries: SpeechFormantBoundaryProfile::new(
            BoundedSequence::try_from_iter([SpeechFormantBoundaryBinding::new(
                SpeechBoundaryKind::Phrase,
                SpeechFormantBoundary::Phrase,
            )
            .unwrap()])
            .unwrap(),
        )
        .unwrap(),
    }
}

/// Explicit reviewed phone data for the v2 profile's surface-preserving lemma.
/// This is supplied pronunciation data, not a renderer spelling heuristic.
#[allow(dead_code)] // Shared fixture: used by the learned acquisition harness.
pub fn parser_phone_profile() -> SpeechPronunciationProfile {
    let original = phones();
    let rows = original.rows().iter().cloned().collect::<Vec<_>>();
    SpeechPronunciationProfile::new(
        "proof/parser-v2-phones".into(),
        original.language().clone(),
        speech_provenance(),
        BoundedSequence::try_from_iter([
            SpeechPronunciationRow::new(
                candidate("Hello", LanguageLexicalPos::Interjection),
                rows[0].phones().clone(),
            )
            .unwrap(),
            rows[1].clone(),
            rows[2].clone(),
        ])
        .unwrap(),
    )
    .unwrap()
}
