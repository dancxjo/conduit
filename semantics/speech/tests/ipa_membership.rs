#![cfg(feature = "semantic-bindings")]
use conduit_language::{LanguageId, LanguageVariety, VarietyId};
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{ipa_admission::*, ipa_notation::*, semantic::*};
fn provenance(note: &str) -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(note.into(), SpeechEvidenceSource::Manual, None).unwrap()
}
fn profile(inventory: &str, variety: &str, revision: &str, note: &str) -> SpeechIpaNotationProfile {
    SpeechIpaNotationProfile::new(
        BoundedSequence::new(),
        SpeechIpaNotationProfileId::new("notation/test@1".into()).unwrap(),
        SpeechInventoryId::new(inventory.into()).unwrap(),
        provenance(note),
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechIpaUnitDefinition::new(
            SpeechIpaUnitId::new("schwa".into()).unwrap(),
            SpeechIpaUnitKind::Segment,
            provenance("unit evidence"),
            SpeechIpaSpelling::new("ə".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        LanguageVariety::new(
            VarietyId::new(variety.into()).unwrap(),
            LanguageId::new("language/test".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn executed_ipa_retains_full_profile_and_refuses_foreign_basis() {
    let original = profile(
        "inventory/a",
        "variety/a",
        "revision/a",
        "original evidence",
    );
    let prepared = PreparedIpaNotationProfile::prepare(&original).unwrap();
    let admitted = admit_ipa_transcription(
        &prepared,
        "[ə]".into(),
        SpeechIpaDisplayKind::Phonetic,
        provenance("authored input"),
    )
    .unwrap();
    admitted.require_profile(&original).unwrap();
    for foreign in [
        profile(
            "inventory/b",
            "variety/a",
            "revision/a",
            "original evidence",
        ),
        profile(
            "inventory/a",
            "variety/b",
            "revision/a",
            "original evidence",
        ),
        profile(
            "inventory/a",
            "variety/a",
            "revision/b",
            "original evidence",
        ),
    ] {
        assert_eq!(admitted.require_profile(&foreign), Err(IpaProfileMismatch));
        assert!(SpeechIpaProfileMatch::new(foreign, admitted.transcription().clone()).is_err());
    }
    // IDs, spelling and revision are unchanged; differing full provenance still
    // cannot be substituted for the exact profile whose parser actually ran.
    let same_ids = profile("inventory/a", "variety/a", "revision/a", "foreign evidence");
    assert_eq!(same_ids.identity(), original.identity());
    assert_eq!(admitted.require_profile(&same_ids), Err(IpaProfileMismatch));
    assert_eq!(admitted.transcription().original(), "[ə]");
}

#[test]
fn duplicate_definitions_and_guessed_aliases_cannot_prepare() {
    let original = profile(
        "inventory/a",
        "variety/a",
        "revision/a",
        "original evidence",
    );
    let rebuild = |aliases, units| {
        SpeechIpaNotationProfile::new(
            aliases,
            original.identity().clone(),
            original.inventory_id().clone(),
            original.provenance().clone(),
            original.revision().clone(),
            units,
            original.variety().clone(),
        )
        .unwrap()
    };
    let duplicates = rebuild(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter([original.units()[0].clone(), original.units()[0].clone()])
            .unwrap(),
    );
    assert!(matches!(
        PreparedIpaNotationProfile::prepare(&duplicates),
        Err(IpaNotationRefusal::InvalidProfile)
    ));
    let guessed = rebuild(
        BoundedSequence::try_from_iter([SpeechIpaSpellingAlias::new(
            provenance("unsupported guessed provider alias"),
            SpeechIpaSpelling::new("ax".into()).unwrap(),
            original.units()[0].identity().clone(),
        )
        .unwrap()])
        .unwrap(),
        original.units().clone(),
    );
    assert!(matches!(
        PreparedIpaNotationProfile::prepare(&guessed),
        Err(IpaNotationRefusal::InvalidProfile)
    ));
}
