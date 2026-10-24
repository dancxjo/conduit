//! G1@1: manually supplied /tɑtɑ/ words; initial /t/ aspirates, medial /t/ does not.
//! This declares facts, not a text pronunciation rule or a universal language claim.
use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
pub fn seq<T, const N: usize>(v: impl IntoIterator<Item = T>) -> BoundedSequence<T, N> {
    BoundedSequence::try_from_iter(v).ok().unwrap()
}
pub fn evidence() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "PhonoGestureG1 declared profile".into(),
        SpeechEvidenceSource::Manual,
        Some("1".into()),
    )
    .unwrap()
}
pub fn confidence() -> SpeechConfidence {
    SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap()
}
pub fn language() -> LanguageId {
    LanguageId::new("language/en".into()).unwrap()
}
pub fn basis(sequence: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("g1/tata@1".into()).unwrap(),
        language(),
        SpeechSegmentRevisionId::new("g1/occurrences/1".into()).unwrap(),
        SpeechSegmentSequenceId::new(sequence.into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
pub fn occurrence(sequence: &str, n: u32) -> LanguageSpeechTokenRef {
    let b = basis(sequence);
    LanguageSpeechTokenRef::new(
        b.inventory_id().clone(),
        b.language().clone(),
        n,
        b.revision_id().clone(),
        b.sequence_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap()
}
pub fn source() -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("utterance".into()).unwrap(),
            language(),
            LanguageTextRevisionId::new("source/1".into()).unwrap(),
            "tata tata tata".into(),
        )
        .unwrap(),
        None,
        LinguisticDerivationProvenance::deterministic_rule(
            "g1/manual-lexical-material".into(),
            "1".into(),
        )
        .unwrap(),
        0,
        Some(14),
    )
    .unwrap()
}
pub fn text_ref(kind: LanguageTextSegmentKind, start: u32, end: u32) -> LanguageTextSegmentRef {
    LanguageTextSegmentRef::new(
        kind,
        language(),
        LanguageTextRange::new(end, start).unwrap(),
        source().material().revision().clone(),
        source().material().identity().clone(),
    )
    .unwrap()
}
pub fn word(n: usize) -> LanguageTextSegmentRef {
    text_ref(
        LanguageTextSegmentKind::Word,
        n as u32 * 5,
        n as u32 * 5 + 4,
    )
}
pub fn lexical_profile() -> LanguageLexicalProfile {
    LanguageLexicalProfile::new(
        seq([LanguageLexicalEntry::new(
            seq([
                LanguageLexicalCandidate::new("tata".into(), seq([]), LanguageLexicalPos::Noun)
                    .unwrap(),
            ]),
            "tata".into(),
        )
        .unwrap()]),
        "g1/manual-lexical@1".into(),
        language(),
        source().provenance().clone(),
    )
    .unwrap()
}
pub fn pid(s: &str) -> PhoneId {
    PhoneId::new(format!("g1/{s}")).unwrap()
}
pub fn phoneme_id(n: usize) -> PhonemeId {
    PhonemeId::new(
        if n.is_multiple_of(2) {
            "g1/phoneme/t"
        } else {
            "g1/phoneme/a"
        }
        .into(),
    )
    .unwrap()
}
pub fn phone_name(n: usize) -> &'static str {
    match n % 4 {
        0 => "aspirated-t",
        2 => "t",
        _ => "a",
    }
}
pub fn stress(n: usize) -> StressSpecification {
    StressSpecification::known(if n % 4 < 2 {
        SpeechStress::Primary
    } else {
        SpeechStress::Unstressed
    })
    .unwrap()
}
pub fn inventory(unsupported: bool) -> SpeechInventory {
    let conditions = if unsupported {
        vec![SpeechRuleCondition::current_word_has_syntactic_link(
            SpeechSyntacticLinkKind::Vocative,
        )
        .unwrap()]
    } else {
        vec![]
    };
    let rule = SpeechPhonemeAllophone::new(
        seq(conditions),
        confidence(),
        SpeechEnvironment::new(
            seq([]),
            seq([]),
            SpeechProsodicContextSpecification::unspecified(),
            StressSpecification::unspecified(),
            SpeechSyllablePositionSpecification::unspecified(),
            SpeechPositionSpecification::known(SpeechWordPosition::Initial).unwrap(),
        )
        .unwrap(),
        pid("aspirated-t"),
        Some("G1@1 word-initial aspiration".into()),
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    SpeechInventory::new(
        basis("phones").inventory_id().clone(),
        language(),
        seq([("t", vec![rule]), ("a", vec![])].map(|(id, rules)| {
            SpeechPhoneme::new(
                seq([]),
                seq(rules),
                Some(pid(id)),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeId::new(format!("g1/phoneme/{id}")).unwrap(),
                if id == "a" { "ɑ" } else { id }.into(),
                seq([]),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })),
        seq(
            [("t", "t"), ("aspirated-t", "tʰ"), ("a", "ɑ")].map(|(id, ipa)| {
                SpeechPhone::new(
                    seq([]),
                    SpeechFeatureBundle::new(seq([])).unwrap(),
                    pid(id),
                    ipa.into(),
                    SpeechSegmentStatus::Core,
                )
                .unwrap()
            }),
        ),
    )
    .unwrap()
}
pub fn profile() -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        seq([]),
        SpeechIpaNotationProfileId::new("g1/contextual-ipa@1".into()).unwrap(),
        basis("phones").inventory_id().clone(),
        evidence(),
        SpeechSegmentRevisionId::new("g1/inventory/1".into()).unwrap(),
        seq(
            [("t", "t"), ("aspiration", "tʰ"), ("a", "ɑ")].map(|(id, spelling)| {
                SpeechIpaUnitDefinition::new(
                    SpeechIpaUnitId::new(id.into()).unwrap(),
                    SpeechIpaUnitKind::Segment,
                    evidence(),
                    SpeechIpaSpelling::new(spelling.into()).unwrap(),
                )
                .unwrap()
            }),
        ),
        LanguageVariety::new(
            VarietyId::new("g1/declared-careful-tata".into()).unwrap(),
            language(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn phone_bindings() -> Vec<SpeechIpaPhoneDefinitionBinding> {
    [
        ("t", vec!["t"]),
        ("aspirated-t", vec!["aspiration"]),
        ("a", vec!["a"]),
    ]
    .into_iter()
    .map(|(id, units)| {
        SpeechIpaPhoneDefinitionBinding::new(
            pid(id),
            evidence(),
            seq(units
                .into_iter()
                .map(|s| SpeechIpaUnitId::new(s.into()).unwrap())),
        )
        .unwrap()
    })
    .collect()
}
pub fn phoneme_bindings() -> Vec<SpeechIpaPhonemeDefinitionBinding> {
    ["t", "a"]
        .map(|id| {
            SpeechIpaPhonemeDefinitionBinding::new(
                PhonemeId::new(format!("g1/phoneme/{id}")).unwrap(),
                evidence(),
                seq([SpeechIpaUnitId::new(id.into()).unwrap()]),
            )
            .unwrap()
        })
        .to_vec()
}
pub fn formant(inv: &SpeechInventory) -> SpeechFormantVoiceProfile {
    SpeechFormantVoiceProfile::new(
        "g1/formant@1".into(),
        inv.identity().clone(),
        language(),
        seq(inv
            .phones()
            .as_slice()
            .iter()
            .cloned()
            .zip([EnglishPhone::T, EnglishPhone::TAspirated, EnglishPhone::Aa])
            .map(|(d, p)| SpeechFormantPhoneBinding::new(d, p).unwrap())),
    )
    .unwrap()
}
