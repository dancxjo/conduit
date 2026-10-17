//! Actual owner Types used by the private capacity compatibility witness.
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;

pub fn provenance(note: &str) -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(note.into(), SpeechEvidenceSource::Manual, None).unwrap()
}
pub fn inventory() -> SpeechInventory {
    let phone = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("phone/t".into()).unwrap(),
        "t͡ʃ".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::new(),
        None,
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/t".into()).unwrap(),
        "t͡ʃ".into(),
        BoundedSequence::new(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory".into()).unwrap(),
        LanguageId::new("en".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([phone]).unwrap(),
    )
    .unwrap()
}
pub fn profile() -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        BoundedSequence::new(),
        SpeechIpaNotationProfileId::new("notation/capacity-witness@1".into()).unwrap(),
        SpeechInventoryId::new("inventory".into()).unwrap(),
        provenance("whole original notation profile"),
        SpeechSegmentRevisionId::new("revision".into()).unwrap(),
        BoundedSequence::try_from_iter([
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new("affricate".into()).unwrap(),
                SpeechIpaUnitKind::Segment,
                provenance("authored affricate"),
                SpeechIpaSpelling::new("t͡ʃ".into()).unwrap(),
            )
            .unwrap(),
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new("syllable-boundary".into()).unwrap(),
                SpeechIpaUnitKind::SyllableBoundary,
                provenance("authored notation boundary; not inferred syllable structure"),
                SpeechIpaSpelling::new(".".into()).unwrap(),
            )
            .unwrap(),
        ])
        .unwrap(),
        LanguageVariety::new(
            VarietyId::new("variety/capacity-witness".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn phone_binding() -> SpeechIpaPhoneDefinitionBinding {
    SpeechIpaPhoneDefinitionBinding::new(
        PhoneId::new("phone/t".into()).unwrap(),
        provenance("phone spelling, not a phoneme equivalence"),
        BoundedSequence::try_from_iter([SpeechIpaUnitId::new("affricate".into()).unwrap()])
            .unwrap(),
    )
    .unwrap()
}
pub fn phoneme_binding() -> SpeechIpaPhonemeDefinitionBinding {
    SpeechIpaPhonemeDefinitionBinding::new(
        PhonemeId::new("phoneme/t".into()).unwrap(),
        provenance("phoneme spelling, not a realization"),
        BoundedSequence::try_from_iter([SpeechIpaUnitId::new("affricate".into()).unwrap()])
            .unwrap(),
    )
    .unwrap()
}
pub fn revision(previous: Option<&LanguageTextRevision>, content: &str) -> LanguageTextRevision {
    let sequence = previous.map_or(0, |value| *value.sequence() + 1);
    let revision = if sequence == 0 {
        "source revision".into()
    } else {
        format!("source revision/{sequence}")
    };
    LanguageTextRevision::new(
        LanguageTextFinality::Partial,
        LanguageText::new(
            LanguageTextId::new("source".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
            LanguageTextRevisionId::new(revision).unwrap(),
            content.into(),
        )
        .unwrap(),
        previous.map(|value| {
            LanguageTextPriorRevision::new(value.material().revision().clone(), *value.sequence())
                .unwrap()
        }),
        LinguisticDerivationProvenance::deterministic_rule(
            "capacity-custody-witness".into(),
            "1".into(),
        )
        .unwrap(),
        sequence,
        Some(content.chars().count() as u32),
    )
    .unwrap()
}
