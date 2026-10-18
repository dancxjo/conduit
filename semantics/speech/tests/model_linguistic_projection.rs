#![cfg(feature = "semantic-bindings")]
#[path = "common/correspondence.rs"]
mod fixture;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    correspondence::PreparedSpeechCorrespondence, ipa_inventory::PreparedIpaInventory,
    model_linguistic_projection::*, semantic::*,
};

fn vocabulary(
    basis: &SpeechIpaInventoryNotationBasis,
    token: u32,
) -> SpeechModelLinguisticVocabulary {
    SpeechModelLinguisticVocabulary::new(
        basis.clone(),
        SpeechModelVocabularyId::new("test/lexical-v1".into()).unwrap(),
        BoundedSequence::try_from_iter([SpeechModelPhonemeToken::new(
            PhonemeId::new("phoneme/t".into()).unwrap(),
            42,
        )
        .unwrap()])
        .unwrap(),
        BoundedSequence::try_from_iter([SpeechModelPhoneToken::new(
            PhoneId::new("phone/tap".into()).unwrap(),
            token,
        )
        .unwrap()])
        .unwrap(),
        SpeechModelProjectionVersion::new(1).unwrap(),
    )
    .unwrap()
}

#[test]
fn shared_lexical_map_preserves_distinct_streams_and_original_through_backend_refusal() {
    let (definitions, profile, text, intent, phonemes, phones, alignment) = (
        fixture::inventory(),
        fixture::profile(),
        fixture::text(),
        fixture::intent(),
        fixture::phonemes(),
        fixture::phones(),
        fixture::alignment(fixture::groups()),
    );
    let (phone_binding, phoneme_binding) = fixture::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &definitions,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let original_bytes = intent.clone().encode().unwrap();
    let vocabulary = vocabulary(inventory.basis(), 17);
    let checkpoint = SpeechModelCheckpointProjection::new(
        SpeechModelCheckpointId::new("synthetic/compatibility-only".into()).unwrap(),
        vocabulary.clone(),
    )
    .unwrap();
    let decoded =
        SpeechModelCheckpointProjection::decode(&checkpoint.clone().encode().unwrap()).unwrap();
    let training = prepare_model_training_input(&original, &vocabulary).unwrap();
    let inference =
        prepare_model_inference_input(&original, &vocabulary, &decoded, decoded.checkpoint())
            .unwrap();
    assert!(core::ptr::eq(training.original(), &original));
    assert!(core::ptr::eq(inference.projection().original(), &original));
    assert_eq!(
        training.phone_tokens(),
        inference.projection().phone_tokens()
    );
    assert_eq!(
        training.phoneme_tokens(),
        inference.projection().phoneme_tokens()
    );
    assert_eq!(training.phone_tokens().len(), phones.tokens().len());
    assert_eq!(training.phoneme_tokens().len(), phonemes.tokens().len());
    assert!(training
        .phone_tokens()
        .iter()
        .all(|token| token.backend_value() == 17));
    assert!(training
        .phoneme_tokens()
        .iter()
        .all(|token| token.backend_value() == 42));
    // Non-bijective aligned/deleted/inserted relations remain solely on original.
    assert!(core::ptr::eq(training.original().alignment(), &alignment));
    assert!(core::ptr::eq(training.original().inventory(), &inventory));
    let refusal = inference.refuse_unsupported_backend();
    assert_eq!(refusal.reason, ModelBackendRefusal::NoAdmittedAcousticBack);
    assert!(core::ptr::eq(
        refusal.input.projection().original().intent(),
        &intent
    ));
    assert_eq!(intent.clone().encode().unwrap(), original_bytes);
    // Same label, changed content must not pass checkpoint compatibility.
    let stale = vocabulary_for_token(inventory.basis(), 18);
    assert!(
        prepare_model_inference_input(&original, &stale, &decoded, decoded.checkpoint()).is_err()
    );
    let foreign_checkpoint = SpeechModelCheckpointId::new("another/checkpoint".into()).unwrap();
    assert!(matches!(
        prepare_model_inference_input(&original, &vocabulary, &decoded, &foreign_checkpoint),
        Err(ModelProjectionRefusal::Checkpoint)
    ));
    // No schema-version upgrade is inferred.
    assert!(SpeechModelProjectionVersion::new(2).is_err());
    // Same IDs under another exact inventory revision are ineligible.
    let stale_basis = SpeechIpaInventoryNotationBasis::new(
        inventory.basis().inventory_id().clone(),
        inventory.basis().language().clone(),
        inventory.basis().profile().clone(),
        SpeechSegmentRevisionId::new("stale/revision".into()).unwrap(),
        inventory.basis().variety().clone(),
    );
    assert!(stale_basis.is_err());
}

fn vocabulary_for_token(
    basis: &SpeechIpaInventoryNotationBasis,
    token: u32,
) -> SpeechModelLinguisticVocabulary {
    vocabulary(basis, token)
}

#[test]
fn executed_compatibility_checks_complete_content_and_reloaded_values() {
    let profile = fixture::profile();
    let basis = SpeechIpaInventoryNotationBasis::new(
        profile.inventory_id().clone(),
        profile.variety().language().clone(),
        profile.clone(),
        profile.revision().clone(),
        profile.variety().clone(),
    )
    .unwrap();
    let first = vocabulary(&basis, 1);
    let same = SpeechModelLinguisticVocabulary::decode(&first.clone().encode().unwrap()).unwrap();
    SpeechModelProjectionCompatibility::new(first.clone(), same).unwrap();
    assert!(SpeechModelProjectionCompatibility::new(first, vocabulary(&basis, 2)).is_err());
}

#[test]
fn duplicate_and_foreign_vocabulary_entries_refuse_before_terminal_input() {
    let (definitions, profile, text, intent, phonemes, phones, alignment) = (
        fixture::inventory(),
        fixture::profile(),
        fixture::text(),
        fixture::intent(),
        fixture::phonemes(),
        fixture::phones(),
        fixture::alignment(fixture::groups()),
    );
    let (phone_binding, phoneme_binding) = fixture::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &definitions,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let valid = vocabulary(inventory.basis(), 17);
    let duplicate = SpeechModelLinguisticVocabulary::new(
        valid.basis().clone(),
        valid.identity().clone(),
        valid.phonemes().clone(),
        BoundedSequence::try_from_iter([valid.phones()[0].clone(), valid.phones()[0].clone()])
            .unwrap(),
        valid.version().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_model_training_input(&original, &duplicate),
        Err(ModelProjectionRefusal::DuplicatePhone)
    ));
    let foreign = SpeechModelLinguisticVocabulary::new(
        valid.basis().clone(),
        valid.identity().clone(),
        valid.phonemes().clone(),
        BoundedSequence::try_from_iter([SpeechModelPhoneToken::new(
            PhoneId::new("foreign/phone".into()).unwrap(),
            99,
        )
        .unwrap()])
        .unwrap(),
        valid.version().clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_model_training_input(&original, &foreign),
        Err(ModelProjectionRefusal::ForeignPhone)
    ));
    // Valid checked basis but another notation-profile identity is a different
    // declared vocabulary, even with unchanged token spellings and inventory ID.
    let alternate_profile = SpeechIpaNotationProfile::new(
        profile.aliases().clone(),
        SpeechIpaNotationProfileId::new("alternate/profile".into()).unwrap(),
        profile.inventory_id().clone(),
        profile.provenance().clone(),
        profile.revision().clone(),
        profile.units().clone(),
        profile.variety().clone(),
    )
    .unwrap();
    let alternate_basis = SpeechIpaInventoryNotationBasis::new(
        alternate_profile.inventory_id().clone(),
        valid.basis().language().clone(),
        alternate_profile.clone(),
        alternate_profile.revision().clone(),
        alternate_profile.variety().clone(),
    )
    .unwrap();
    let swapped = vocabulary(&alternate_basis, 17);
    assert!(matches!(
        prepare_model_training_input(&original, &swapped),
        Err(ModelProjectionRefusal::InventoryBasis)
    ));
}
