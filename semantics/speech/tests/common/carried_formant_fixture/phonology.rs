//! Exact independently supplied inventory, notation, tapes and correspondence.
use super::*;
pub fn inventory() -> SpeechInventory {
    SpeechInventory::new(
        basis("phones").inventory_id().clone(),
        source().material().language().clone(),
        seq(UNITS.map(|(id, ipa)| {
            SpeechPhoneme::new(
                seq([]),
                seq([]),
                None,
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeId::new(format!("contrast/{id}")).unwrap(),
                ipa.into(),
                seq([]),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })),
        seq(UNITS.map(|(id, ipa)| {
            SpeechPhone::new(
                seq([]),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneId::new(format!("target/{id}")).unwrap(),
                ipa.into(),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })),
    )
    .unwrap()
}
pub fn profile() -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        seq([]),
        SpeechIpaNotationProfileId::new("authored/you-would/ipa@1".into()).unwrap(),
        basis("phones").inventory_id().clone(),
        evidence(),
        SpeechSegmentRevisionId::new("inventory/1".into()).unwrap(),
        seq(UNITS.map(|(id, ipa)| {
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new(id.into()).unwrap(),
                SpeechIpaUnitKind::Segment,
                evidence(),
                SpeechIpaSpelling::new(ipa.into()).unwrap(),
            )
            .unwrap()
        })),
        LanguageVariety::new(
            VarietyId::new("authored/broad-you-would".into()).unwrap(),
            source().material().language().clone(),
        )
        .unwrap(),
    )
    .unwrap()
}
pub fn phone_bindings() -> Vec<SpeechIpaPhoneDefinitionBinding> {
    UNITS
        .map(|(id, _)| {
            SpeechIpaPhoneDefinitionBinding::new(
                PhoneId::new(format!("target/{id}")).unwrap(),
                evidence(),
                seq([SpeechIpaUnitId::new(id.into()).unwrap()]),
            )
            .unwrap()
        })
        .to_vec()
}
pub fn phoneme_bindings() -> Vec<SpeechIpaPhonemeDefinitionBinding> {
    UNITS
        .map(|(id, _)| {
            SpeechIpaPhonemeDefinitionBinding::new(
                PhonemeId::new(format!("contrast/{id}")).unwrap(),
                evidence(),
                seq([SpeechIpaUnitId::new(id.into()).unwrap()]),
            )
            .unwrap()
        })
        .to_vec()
}
pub fn phones() -> SpeechPhoneSequence {
    SpeechPhoneSequence::new(
        basis("phones"),
        seq(PHONES.map(|id| {
            SpeechPhoneToken::new(
                seq([]),
                confidence(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneSpecification::known(PhoneId::new(id.into()).unwrap()).unwrap(),
                evidence(),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap()
}
pub fn phonemes() -> SpeechPhonemeSequence {
    SpeechPhonemeSequence::new(
        basis("phonemes"),
        seq(PHONEMES.map(|id| {
            SpeechPhonemeToken::new(
                confidence(),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeSpecification::known(PhonemeId::new(id.into()).unwrap()).unwrap(),
                evidence(),
                seq([]),
                None,
            )
            .unwrap()
        })),
    )
    .unwrap()
}
pub fn alignment() -> SpeechRealizationAlignment {
    let p = profile();
    SpeechRealizationAlignment::new(
        seq((0..5).map(|n| {
            SpeechRealizationCorrespondence::aligned(
                seq([occurrence("phonemes", n)]),
                seq([occurrence("phones", n)]),
            )
            .unwrap()
        })),
        SpeechIpaInventoryNotationBasis::new(
            basis("phones").inventory_id().clone(),
            source().material().language().clone(),
            p.clone(),
            p.revision().clone(),
            p.variety().clone(),
        )
        .unwrap(),
        basis("phones"),
        basis("phonemes"),
        evidence(),
        text_ref(LanguageTextSegmentKind::Utterance, 0, 9),
    )
    .unwrap()
}
