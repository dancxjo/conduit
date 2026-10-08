#![cfg(feature = "semantic-bindings")]
use conduit_language::{LanguageId, LanguageVariety, VarietyId};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{ipa_notation::*, semantic::*};
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "IPA conformance authored fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
fn profile() -> SpeechIpaNotationProfile {
    let definitions = [
        ("affricate", "t͡ʃ", SpeechIpaUnitKind::Segment),
        ("aspiration", "pʰ", SpeechIpaUnitKind::Segment),
        ("syllabic", "n̩", SpeechIpaUnitKind::Segment),
        ("nasal", "ã", SpeechIpaUnitKind::Segment),
        ("stress", "ˈ", SpeechIpaUnitKind::PrimaryStress),
        ("secondary", "ˌ", SpeechIpaUnitKind::SecondaryStress),
        ("length", "ː", SpeechIpaUnitKind::Length),
        ("boundary", ".", SpeechIpaUnitKind::SyllableBoundary),
    ];
    let units = definitions.into_iter().map(|(id, spelling, kind)| {
        SpeechIpaUnitDefinition::new(
            SpeechIpaUnitId::new(id.into()).unwrap(),
            kind,
            provenance(),
            SpeechIpaSpelling::new(spelling.into()).unwrap(),
        )
        .unwrap()
    });
    SpeechIpaNotationProfile::new(
        BoundedSequence::try_from_iter([SpeechIpaSpellingAlias::new(
            provenance(),
            SpeechIpaSpelling::new("ã".into()).unwrap(),
            SpeechIpaUnitId::new("nasal".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        SpeechIpaNotationProfileId::new("notation/conservative-unicode@1".into()).unwrap(),
        SpeechInventoryId::new("inventory/conformance".into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new("revision/1".into()).unwrap(),
        BoundedSequence::try_from_iter(units).unwrap(),
        LanguageVariety::new(
            VarietyId::new("variety/conformance".into()).unwrap(),
            LanguageId::new("language/conformance".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn checked_unicode_profile_retains_original_spelling_and_all_unit_spans() {
    let profile = profile();
    let prepared = PreparedIpaNotationProfile::prepare(&profile).unwrap();
    for (text, display) in [
        ("[ˈt͡ʃpʰn̩ããː]", SpeechIpaDisplayKind::Phonetic),
        ("/ˌt͡ʃ.n̩ː/", SpeechIpaDisplayKind::Phonemic),
    ] {
        let receipt = prepared.parse(text.into(), display, provenance()).unwrap();
        let encoded = receipt.clone().encode().unwrap();
        let decoded = SpeechIpaProfileMatch::decode(&encoded).unwrap();
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.profile(), &profile);
        assert_eq!(decoded.transcription().original(), text);
        let mut cursor = 1;
        for occurrence in decoded.transcription().units().as_slice() {
            assert_eq!(*occurrence.start(), cursor);
            assert_eq!(
                &text[*occurrence.start() as usize..*occurrence.end() as usize],
                occurrence.source_spelling().get().as_str()
            );
            cursor = *occurrence.end();
        }
        assert_eq!(cursor as usize, text.len() - 1);
    }
}
#[test]
fn unsupported_notation_delimiters_and_suprasegmental_order_refuse() {
    let profile = profile();
    let prepared = PreparedIpaNotationProfile::prepare(&profile).unwrap();
    for text in [
        "[ch]", "[tʃ]", "[ːã]", "[ãːː]", "[ˈ]", "[.ã]", "[ã..ã]", "[ã.]", "[ˈˌã]", "[a]",
    ] {
        assert!(
            prepared
                .parse(text.into(), SpeechIpaDisplayKind::Phonetic, provenance())
                .is_err(),
            "{text}"
        );
    }
    assert!(prepared
        .parse("/ã/".into(), SpeechIpaDisplayKind::Phonetic, provenance())
        .is_err());
}
