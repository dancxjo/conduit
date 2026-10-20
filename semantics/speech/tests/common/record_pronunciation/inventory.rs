//! Finite independently named broad UK dictionary inventory, never you-would.
use conduit_language::{LanguageId, LanguageVariety, VarietyId};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{record_pronunciation::RECORD_VARIETY_ID, semantic::*};
fn seq<T, const N: usize>(v: impl IntoIterator<Item = T>) -> BoundedSequence<T, N> {
    BoundedSequence::try_from_iter(v).ok().unwrap()
}
pub fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "Cambridge UK record/a/we; explicit reviewed fixture".into(),
        SpeechEvidenceSource::Manual,
        Some("1".into()),
    )
    .unwrap()
}
pub struct Inventory {
    pub inventory: SpeechInventory,
    pub profile: SpeechIpaNotationProfile,
    pub phones: Vec<SpeechIpaPhoneDefinitionBinding>,
    pub phonemes: Vec<SpeechIpaPhonemeDefinitionBinding>,
}
pub fn inventory(variety: &str) -> Inventory {
    let language = LanguageId::new("language/en".into()).unwrap();
    let id = SpeechInventoryId::new("speech/uk-record/inventory@1".into()).unwrap();
    let definitions = [
        ("r", "r", vec!["r"]),
        ("e", "e", vec!["e"]),
        ("ih", "ɪ", vec!["ih"]),
        ("k", "k", vec!["k"]),
        ("ao-long", "ɔː", vec!["ao", "length"]),
        ("d", "d", vec!["d"]),
        ("ax", "ə", vec!["ax"]),
        ("w", "w", vec!["w"]),
        ("i", "i", vec!["i"]),
    ];
    let inventory = SpeechInventory::new(
        id.clone(),
        language.clone(),
        seq(definitions.iter().map(|(name, ipa, _)| {
            SpeechPhoneme::new(
                seq([]),
                seq([]),
                None,
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhonemeId::new(format!("uk-record/{name}")).unwrap(),
                (*ipa).into(),
                seq([]),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })),
        seq(PHONES.iter().map(|(name, ipa, _)| {
            SpeechPhone::new(
                seq([]),
                SpeechFeatureBundle::new(seq([])).unwrap(),
                PhoneId::new(format!("uk-record/phone/{name}")).unwrap(),
                (*ipa).into(),
                SpeechSegmentStatus::Core,
            )
            .unwrap()
        })),
    )
    .unwrap();
    let units = [
        ("r", "r", SpeechIpaUnitKind::Segment),
        ("e", "e", SpeechIpaUnitKind::Segment),
        ("ih", "ɪ", SpeechIpaUnitKind::Segment),
        ("k", "k", SpeechIpaUnitKind::Segment),
        ("ao", "ɔ", SpeechIpaUnitKind::Segment),
        ("d", "d", SpeechIpaUnitKind::Segment),
        ("ax", "ə", SpeechIpaUnitKind::Segment),
        ("w", "w", SpeechIpaUnitKind::Segment),
        ("i", "i", SpeechIpaUnitKind::Segment),
        ("rhotic", "ɹ", SpeechIpaUnitKind::Segment),
        ("eh", "ɛ", SpeechIpaUnitKind::Segment),
        ("length", "ː", SpeechIpaUnitKind::Length),
        ("stress", "ˈ", SpeechIpaUnitKind::PrimaryStress),
        ("boundary", ".", SpeechIpaUnitKind::SyllableBoundary),
    ];
    let profile = SpeechIpaNotationProfile::new(
        seq([]),
        SpeechIpaNotationProfileId::new("speech/uk-record/notation@1".into()).unwrap(),
        id,
        provenance(),
        SpeechSegmentRevisionId::new("uk-record/revision@1".into()).unwrap(),
        seq(units.map(|(name, ipa, kind)| {
            SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new(name.into()).unwrap(),
                kind,
                provenance(),
                SpeechIpaSpelling::new(ipa.into()).unwrap(),
            )
            .unwrap()
        })),
        LanguageVariety::new(VarietyId::new(variety.into()).unwrap(), language).unwrap(),
    )
    .unwrap();
    let phonemes = definitions
        .iter()
        .map(|(name, _, units)| {
            SpeechIpaPhonemeDefinitionBinding::new(
                PhonemeId::new(format!("uk-record/{name}")).unwrap(),
                provenance(),
                seq(units
                    .iter()
                    .map(|u| SpeechIpaUnitId::new((*u).into()).unwrap())),
            )
            .unwrap()
        })
        .collect();
    let phones = PHONES
        .iter()
        .map(|(name, _, unit)| {
            SpeechIpaPhoneDefinitionBinding::new(
                PhoneId::new(format!("uk-record/phone/{name}")).unwrap(),
                provenance(),
                seq([SpeechIpaUnitId::new((*unit).into()).unwrap()]),
            )
            .unwrap()
        })
        .collect();
    Inventory {
        inventory,
        phones,
        profile,
        phonemes,
    }
}
pub fn reviewed() -> Inventory {
    inventory(RECORD_VARIETY_ID)
}

/// Explicit reviewed realization target; phonemic r/e remain separate originals.
pub const PHONES: [(&str, &str, &str); 9] = [
    ("r", "ɹ", "rhotic"),
    ("eh", "ɛ", "eh"),
    ("ih", "ɪ", "ih"),
    ("k", "k", "k"),
    ("ao", "ɔ", "ao"),
    ("d", "d", "d"),
    ("ax", "ə", "ax"),
    ("w", "w", "w"),
    ("i", "i", "i"),
];
