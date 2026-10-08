use conduit_core::IeeeF32;
use conduit_language::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::semantic::*;
pub struct InventoryFixture {
    pub inventory: SpeechInventory,
    pub notation: SpeechIpaNotationProfile,
    pub phone_notations: Vec<SpeechIpaPhoneDefinitionBinding>,
    pub phoneme_notations: Vec<SpeechIpaPhonemeDefinitionBinding>,
}
pub fn inventory(
    composite: &SpeechUtteranceIntent,
    provenance: &SpeechEvidenceProvenance,
) -> InventoryFixture {
    let ipa_symbols = ["h", "ə", "l", "oʊ", "t", "ɹ", "æ", "v", "ɪ", "s"];
    let phone_id = |ipa: &str| PhoneId::new(format!("phone/{ipa}")).unwrap();
    let phoneme_id = |ipa: &str| PhonemeId::new(format!("phoneme/{ipa}")).unwrap();
    let confidence = || SpeechConfidence::new(IeeeF32::from_value(1.0)).unwrap();
    // Reviewed fixture data, scoped to this profile. These are descriptive
    // segment features, not anatomy or a universal symbol inference rule.
    let feature_rows = [
        ("h", "consonant", "voiceless", "unaspirated"),
        ("ə", "vowel", "voiced", "unaspirated"),
        ("l", "consonant", "voiced", "unaspirated"),
        ("oʊ", "vowel", "voiced", "unaspirated"),
        ("t", "consonant", "voiceless", "unaspirated"),
        ("ɹ", "consonant", "voiced", "unaspirated"),
        ("æ", "vowel", "voiced", "unaspirated"),
        ("v", "consonant", "voiced", "unaspirated"),
        ("ɪ", "vowel", "voiced", "unaspirated"),
        ("s", "consonant", "voiceless", "unaspirated"),
        ("tʰ", "consonant", "voiceless", "aspirated"),
    ];
    let defined_features = |ipa: &str| {
        let (_, class, voice, aspiration) = feature_rows.iter().find(|row| row.0 == ipa).unwrap();
        SpeechFeatureBundle::new(
            BoundedSequence::try_from_iter(
                [
                    ("segment-class", *class),
                    ("laryngeal-voice", *voice),
                    ("aspiration", *aspiration),
                ]
                .map(|(name, value)| {
                    SpeechFeature::new(
                        SpeechFeatureId::new(format!("reviewed/greeting/{name}")).unwrap(),
                        FeatureSpecification::known(
                            SpeechFeatureValue::category(value.into()).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap()
                }),
            )
            .unwrap(),
        )
        .unwrap()
    };

    let provenance = provenance.clone();
    let aspiration_rule = SpeechPhonemeAllophone::new(
        BoundedSequence::new(),
        confidence(),
        SpeechEnvironment::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            SpeechProsodicContextSpecification::Unspecified,
            StressSpecification::known(SpeechStress::Primary).unwrap(),
            SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Onset).unwrap(),
            SpeechPositionSpecification::Unspecified,
        )
        .unwrap(),
        phone_id("tʰ"),
        Some("reviewed/greeting/stressed-onset-aspiration-v1".into()),
        SpeechRuleStatus::Productive,
    )
    .unwrap();
    let phone_definitions = ipa_symbols
        .iter()
        .copied()
        .chain(["tʰ"])
        .map(|ipa| {
            SpeechPhone::new(
                BoundedSequence::new(),
                defined_features(ipa),
                phone_id(ipa),
                ipa.into(),
                if ipa == "tʰ" {
                    SpeechSegmentStatus::Allophonic
                } else {
                    SpeechSegmentStatus::Core
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let phoneme_definitions = ipa_symbols
        .iter()
        .map(|ipa| {
            SpeechPhoneme::new(
                BoundedSequence::new(),
                BoundedSequence::try_from_iter(if *ipa == "t" {
                    vec![aspiration_rule.clone()]
                } else {
                    vec![]
                })
                .unwrap(),
                Some(phone_id(ipa)),
                defined_features(ipa),
                phoneme_id(ipa),
                (*ipa).into(),
                BoundedSequence::try_from_iter(if *ipa == "t" {
                    vec![phone_id("t"), phone_id("tʰ")]
                } else {
                    vec![phone_id(ipa)]
                })
                .unwrap(),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let linguistic_inventory = SpeechInventory::new(
        composite.inventory_id().clone(),
        composite.language().clone(),
        BoundedSequence::try_from_iter(phoneme_definitions).unwrap(),
        BoundedSequence::try_from_iter(phone_definitions).unwrap(),
    )
    .unwrap();
    let notation = SpeechIpaNotationProfile::new(
        BoundedSequence::new(),
        SpeechIpaNotationProfileId::new("reviewed/greeting/ipa-v1".into()).unwrap(),
        linguistic_inventory.identity().clone(),
        provenance.clone(),
        composite.revision_id().clone(),
        BoundedSequence::try_from_iter(
            ipa_symbols
                .iter()
                .copied()
                .filter(|ipa| *ipa != "oʊ")
                .chain(["o", "ʊ", "tʰ"])
                .map(|ipa| {
                    SpeechIpaUnitDefinition::new(
                        SpeechIpaUnitId::new(format!("ipa/{ipa}")).unwrap(),
                        SpeechIpaUnitKind::Segment,
                        provenance.clone(),
                        SpeechIpaSpelling::new(ipa.into()).unwrap(),
                    )
                    .unwrap()
                }),
        )
        .unwrap(),
        LanguageVariety::new(
            VarietyId::new("reviewed/english-greeting-approximation-v1".into()).unwrap(),
            composite.language().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let declared_units = |ipa: &str| {
        BoundedSequence::try_from_iter(if ipa == "oʊ" {
            vec![
                SpeechIpaUnitId::new("ipa/o".into()).unwrap(),
                SpeechIpaUnitId::new("ipa/ʊ".into()).unwrap(),
            ]
        } else {
            vec![SpeechIpaUnitId::new(format!("ipa/{ipa}")).unwrap()]
        })
        .unwrap()
    };
    let phone_notations = linguistic_inventory
        .phones()
        .iter()
        .map(|phone| {
            SpeechIpaPhoneDefinitionBinding::new(
                phone.identity().clone(),
                provenance.clone(),
                declared_units(phone.ipa()),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let phoneme_notations = linguistic_inventory
        .phonemes()
        .iter()
        .map(|phoneme| {
            SpeechIpaPhonemeDefinitionBinding::new(
                phoneme.identity().clone(),
                provenance.clone(),
                declared_units(phoneme.notation()),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    InventoryFixture {
        inventory: linguistic_inventory,
        notation,
        phone_notations,
        phoneme_notations,
    }
}
