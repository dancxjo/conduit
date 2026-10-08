#![cfg(feature = "semantic-bindings")]
use conduit_language::{LanguageId, LanguageVariety, VarietyId};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{ipa_inventory::*, semantic::*};
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

fn inventory(ipa: &str) -> SpeechInventory {
    let phone = SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhoneId::new("phone/distinct".into()).unwrap(),
        ipa.into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap();
    let phoneme = SpeechPhoneme::new(
        BoundedSequence::new(),
        BoundedSequence::new(),
        None,
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeId::new("phoneme/distinct".into()).unwrap(),
        "ə".into(),
        BoundedSequence::new(),
        SpeechSegmentStatus::Core,
    )
    .unwrap();
    SpeechInventory::new(
        SpeechInventoryId::new("inventory/a".into()).unwrap(),
        LanguageId::new("language/test".into()).unwrap(),
        BoundedSequence::try_from_iter([phoneme]).unwrap(),
        BoundedSequence::try_from_iter([phone]).unwrap(),
    )
    .unwrap()
}
fn bindings() -> (
    SpeechIpaPhoneDefinitionBinding,
    SpeechIpaPhonemeDefinitionBinding,
) {
    let units =
        || BoundedSequence::try_from_iter([SpeechIpaUnitId::new("schwa".into()).unwrap()]).unwrap();
    (
        SpeechIpaPhoneDefinitionBinding::new(
            PhoneId::new("phone/distinct".into()).unwrap(),
            provenance("phone notation"),
            units(),
        )
        .unwrap(),
        SpeechIpaPhonemeDefinitionBinding::new(
            PhonemeId::new("phoneme/distinct".into()).unwrap(),
            provenance("phoneme notation"),
            units(),
        )
        .unwrap(),
    )
}
#[test]
fn exact_definitions_keep_distinct_identities_and_executed_notation() {
    let inventory = inventory("ə");
    let profile = profile("inventory/a", "variety/a", "revision/a", "original");
    let (phone, phoneme) = bindings();
    let admitted = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        core::slice::from_ref(&phone),
        core::slice::from_ref(&phoneme),
    )
    .unwrap();
    assert_eq!(admitted.inventory(), &inventory);
    assert_eq!(admitted.phones()[0].0, phone);
    assert_eq!(admitted.phonemes()[0].0, phoneme);
    assert_eq!(admitted.phones()[0].1.transcription().original(), "[ə]");
    assert_eq!(admitted.phonemes()[0].1.transcription().original(), "/ə/");
    assert_eq!(
        SpeechInventory::from_structured(inventory.clone().into_structured().unwrap()).unwrap(),
        inventory
    );
    assert_eq!(
        SpeechIpaPhoneDefinitionBinding::from_structured(phone.clone().into_structured().unwrap())
            .unwrap(),
        phone
    );
}
#[test]
fn foreign_basis_missing_bindings_and_provider_codes_refuse() {
    let inventory = inventory("ə");
    let profile = profile("inventory/a", "variety/a", "revision/a", "original");
    let (phone, phoneme) = bindings();
    for foreign_inventory in [
        SpeechInventory::new(
            SpeechInventoryId::new("foreign/inventory".into()).unwrap(),
            inventory.language().clone(),
            inventory.phonemes().clone(),
            inventory.phones().clone(),
        )
        .unwrap(),
        SpeechInventory::new(
            inventory.identity().clone(),
            LanguageId::new("foreign/language".into()).unwrap(),
            inventory.phonemes().clone(),
            inventory.phones().clone(),
        )
        .unwrap(),
    ] {
        assert!(matches!(
            PreparedIpaInventory::prepare(
                &foreign_inventory,
                &profile,
                profile.variety(),
                profile.revision(),
                core::slice::from_ref(&phone),
                core::slice::from_ref(&phoneme)
            ),
            Err(IpaInventoryRefusal::Basis)
        ));
    }
    let foreign = LanguageVariety::new(
        VarietyId::new("foreign".into()).unwrap(),
        profile.variety().language().clone(),
    )
    .unwrap();
    assert!(matches!(
        PreparedIpaInventory::prepare(
            &inventory,
            &profile,
            &foreign,
            profile.revision(),
            core::slice::from_ref(&phone),
            core::slice::from_ref(&phoneme)
        ),
        Err(IpaInventoryRefusal::Basis)
    ));
    assert!(matches!(
        PreparedIpaInventory::prepare(
            &inventory,
            &profile,
            profile.variety(),
            &SpeechSegmentRevisionId::new("foreign".into()).unwrap(),
            core::slice::from_ref(&phone),
            core::slice::from_ref(&phoneme)
        ),
        Err(IpaInventoryRefusal::Basis)
    ));
    assert!(matches!(
        PreparedIpaInventory::prepare(
            &inventory,
            &profile,
            profile.variety(),
            profile.revision(),
            &[],
            core::slice::from_ref(&phoneme)
        ),
        Err(IpaInventoryRefusal::BindingCoverage)
    ));
    let wrong = SpeechIpaPhoneDefinitionBinding::new(
        phone.phone().clone(),
        phone.provenance().clone(),
        BoundedSequence::try_from_iter([SpeechIpaUnitId::new("foreign/unit".into()).unwrap()])
            .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        PreparedIpaInventory::prepare(
            &inventory,
            &profile,
            profile.variety(),
            profile.revision(),
            &[wrong],
            core::slice::from_ref(&phoneme)
        ),
        Err(IpaInventoryRefusal::BindingUnits)
    ));
    let provider = self::inventory("ax");
    assert!(matches!(
        PreparedIpaInventory::prepare(
            &provider,
            &profile,
            profile.variety(),
            profile.revision(),
            &[phone],
            &[phoneme]
        ),
        Err(IpaInventoryRefusal::Notation(_))
    ));
}

#[test]
fn phoneme_realization_cannot_reference_a_foreign_phone() {
    let original = inventory("ə");
    let profile = profile("inventory/a", "variety/a", "revision/a", "original");
    let (phone_binding, phoneme_binding) = bindings();
    let definition = &original.phonemes()[0];
    let foreign = PhoneId::new("foreign/phone".into()).unwrap();
    for (default, possible) in [
        (Some(foreign.clone()), BoundedSequence::new()),
        (None, BoundedSequence::try_from_iter([foreign]).unwrap()),
    ] {
        let phoneme = SpeechPhoneme::new(
            definition.aliases().clone(),
            definition.allophones().clone(),
            default,
            definition.features().clone(),
            definition.identity().clone(),
            definition.notation().clone(),
            possible,
            definition.status().clone(),
        )
        .unwrap();
        let inventory = SpeechInventory::new(
            original.identity().clone(),
            original.language().clone(),
            BoundedSequence::try_from_iter([phoneme]).unwrap(),
            original.phones().clone(),
        )
        .unwrap();
        assert!(matches!(
            PreparedIpaInventory::prepare(
                &inventory,
                &profile,
                profile.variety(),
                profile.revision(),
                core::slice::from_ref(&phone_binding),
                core::slice::from_ref(&phoneme_binding),
            ),
            Err(IpaInventoryRefusal::ForeignPhoneReference)
        ));
    }
}

#[test]
fn quoted_phoneme_constructor_resolves_only_with_the_explicit_inventory() {
    let inventory = inventory("ə");
    let profile = profile("inventory/a", "variety/a", "revision/a", "original");
    let (phone, phoneme) = bindings();
    let prepared = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        core::slice::from_ref(&phone),
        core::slice::from_ref(&phoneme),
    )
    .unwrap();
    let admitted = prepared
        .phoneme_from_ipa("ə".into(), provenance("quoted source"))
        .unwrap();
    assert_eq!(admitted.inventory(), &inventory);
    assert_eq!(admitted.definition(), &inventory.phonemes()[0]);
    assert_eq!(admitted.notation().transcription().original(), "/ə/");
    assert_eq!(admitted.basis().variety(), profile.variety());
    assert!(prepared
        .phoneme_from_ipa("ax".into(), provenance("provider code"))
        .is_err());
}
