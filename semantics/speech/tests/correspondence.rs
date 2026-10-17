#![cfg(feature = "semantic-bindings")]
#[path = "common/correspondence.rs"]
mod fixture;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{correspondence::*, ipa_inventory::PreparedIpaInventory, semantic::*};
use fixture::*;

#[test]
fn every_correspondence_shape_re_admits_without_phone_spelling_identity_inference() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        inventory(),
        profile(),
        text(),
        intent(),
        phonemes(),
        phones(),
    );
    let (phone_binding, phoneme_binding) = bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    let alignment = alignment(groups());
    let bytes = alignment.clone().encode().unwrap();
    let decoded = SpeechRealizationAlignment::decode(&bytes).unwrap();
    assert_eq!(decoded, alignment);
    let admitted = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &decoded, &inventory,
    )
    .unwrap();
    assert!(core::ptr::eq(admitted.intent(), &intent));
    assert!(core::ptr::eq(admitted.phonemes(), &phonemes));
    assert!(core::ptr::eq(admitted.phones(), &phones));
    assert!(core::ptr::eq(admitted.text(), &text));
    assert!(core::ptr::eq(admitted.alignment(), &decoded));
    assert!(core::ptr::eq(admitted.inventory(), &inventory));
    assert_eq!(inventory.inventory().phonemes()[0].notation(), "t");
    assert_eq!(inventory.inventory().phones()[0].ipa(), "ɾ");
    assert_eq!(
        phonemes.tokens()[0].phoneme(),
        &PhonemeSpecification::known(PhonemeId::new("phoneme/t".into()).unwrap()).unwrap()
    );
    assert_ne!(inventory.basis().revision(), phonemes.basis().revision_id());
}

#[test]
fn missing_duplicate_foreign_and_dangling_occurrences_refuse() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        inventory(),
        profile(),
        text(),
        intent(),
        phonemes(),
        phones(),
    );
    let (phone_binding, phoneme_binding) = bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    for broken in [
        {
            let mut g = groups();
            g.remove(0);
            g
        },
        {
            let mut g = groups();
            g.push(aligned(&[0], &[0]));
            g
        },
        {
            let mut g = groups();
            g[0] = aligned(&[0], &[99]);
            g
        },
        {
            let mut g = groups();
            g[0] = SpeechRealizationCorrespondence::aligned(
                conduit_plot::rust_binding::BoundedSequence::try_from_iter([reference(
                    "foreign", 0,
                )])
                .unwrap(),
                conduit_plot::rust_binding::BoundedSequence::try_from_iter([reference(
                    "phones", 0,
                )])
                .unwrap(),
            )
            .unwrap();
            g
        },
    ] {
        let candidate = alignment(broken);
        let decoded = SpeechRealizationAlignment::decode(&candidate.encode().unwrap()).unwrap();
        assert!(PreparedSpeechCorrespondence::prepare(
            &intent, &phonemes, &phones, &text, &decoded, &inventory
        )
        .is_err());
    }
}

#[test]
fn swapped_material_and_nonmembers_do_not_gain_membership_from_valid_ipa() {
    let (raw_inventory, profile, text, intent, phonemes, phones) = (
        inventory(),
        profile(),
        text(),
        intent(),
        phonemes(),
        phones(),
    );
    let (phone_binding, phoneme_binding) = bindings();
    let inventory = PreparedIpaInventory::prepare(
        &raw_inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    let alignment = alignment(groups());
    let foreign_text = LanguageText::new(
        text.identity().clone(),
        text.language().clone(),
        LanguageTextRevisionId::new("text/stale".into()).unwrap(),
        text.text().clone(),
    )
    .unwrap();
    assert!(PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &phones,
        &foreign_text,
        &alignment,
        &inventory
    )
    .is_err());
    let foreign_basis = basis("foreign");
    let foreign_phones = SpeechPhoneSequence::new(foreign_basis, phones.tokens().clone()).unwrap();
    assert!(PreparedSpeechCorrespondence::prepare(
        &intent,
        &phonemes,
        &foreign_phones,
        &text,
        &alignment,
        &inventory
    )
    .is_err());
    let mut tokens = phonemes.tokens().as_slice().to_vec();
    let token = &tokens[0];
    tokens[0] = SpeechPhonemeToken::new(
        token.confidence().clone(),
        token.features().clone(),
        PhonemeSpecification::known(PhonemeId::new("ɾ".into()).unwrap()).unwrap(),
        token.provenance().clone(),
        token.realized_as().clone(),
        token.span().clone(),
    )
    .unwrap();
    let forged = SpeechPhonemeSequence::new(
        phonemes.basis().clone(),
        conduit_plot::rust_binding::BoundedSequence::try_from_iter(tokens).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        PreparedSpeechCorrespondence::prepare(
            &intent, &forged, &phones, &text, &alignment, &inventory
        ),
        Err(CorrespondenceRefusal::InventoryMember)
    ));
    let empty_intent = SpeechUtteranceIntent::new(
        conduit_plot::rust_binding::BoundedSequence::new(),
        intent.inventory_id().clone(),
        intent.language().clone(),
        intent.provenance().clone(),
        intent.revision_id().clone(),
        intent.utterance_id().clone(),
    )
    .unwrap();
    assert!(matches!(
        PreparedSpeechCorrespondence::prepare(
            &empty_intent,
            &phonemes,
            &phones,
            &text,
            &alignment,
            &inventory
        ),
        Err(CorrespondenceRefusal::Missing)
    ));
}

#[test]
fn bounded_groups_and_members_cannot_overflow_the_admitted_profile() {
    use conduit_plot::rust_binding::BoundedSequence;
    assert!(SpeechRealizationCorrespondence::aligned(
        BoundedSequence::new(),
        BoundedSequence::try_from_iter([reference("phones", 0)]).unwrap()
    )
    .is_err());
    let excess = BoundedSequence::try_from_iter((0..9).map(|n| reference("phonemes", n)));
    if let Ok(excess) = excess {
        assert!(SpeechRealizationCorrespondence::aligned(
            excess,
            BoundedSequence::try_from_iter([reference("phones", 0)]).unwrap()
        )
        .is_err());
    }
    let excess = BoundedSequence::try_from_iter((0..65).map(|_| aligned(&[0], &[0])));
    if let Ok(excess) = excess {
        assert!(SpeechRealizationAlignment::new(
            excess,
            alignment(groups()).inventory_basis().clone(),
            basis("phones"),
            basis("phonemes"),
            provenance(),
            source()
        )
        .is_err());
    }
    assert!(SpeechRealizationCorrespondence::deleted(
        BoundedSequence::try_from_iter([reference("phonemes", 0)]).unwrap(),
        "".into()
    )
    .is_err());
}

#[test]
fn exact_inventory_revision_and_variety_cannot_be_swapped() {
    let (inventory, original_profile, text, intent, phonemes, phones) = (
        inventory(),
        profile(),
        text(),
        intent(),
        phonemes(),
        phones(),
    );
    let alignment = alignment(groups());
    for (revision, variety) in [
        ("inventory/foreign", "explicit/test-variety"),
        ("inventory/v3", "foreign/variety"),
    ] {
        let profile = SpeechIpaNotationProfile::new(
            original_profile.aliases().clone(),
            original_profile.identity().clone(),
            original_profile.inventory_id().clone(),
            original_profile.provenance().clone(),
            SpeechSegmentRevisionId::new(revision.into()).unwrap(),
            original_profile.units().clone(),
            conduit_language::LanguageVariety::new(
                VarietyId::new(variety.into()).unwrap(),
                original_profile.variety().language().clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let (phone_binding, phoneme_binding) = bindings();
        let prepared = PreparedIpaInventory::prepare(
            &inventory,
            &profile,
            profile.variety(),
            profile.revision(),
            &[phone_binding],
            &[phoneme_binding],
        )
        .unwrap();
        assert!(matches!(
            PreparedSpeechCorrespondence::prepare(
                &intent, &phonemes, &phones, &text, &alignment, &prepared
            ),
            Err(CorrespondenceRefusal::Basis)
        ));
    }
}

#[test]
fn unresolved_and_contradictory_legacy_phoneme_tokens_refuse() {
    use conduit_plot::rust_binding::BoundedSequence;
    let (inventory, profile, text, intent, phonemes, phones) = (
        inventory(),
        profile(),
        text(),
        intent(),
        phonemes(),
        phones(),
    );
    let (phone_binding, phoneme_binding) = bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[phone_binding],
        &[phoneme_binding],
    )
    .unwrap();
    let alignment = alignment(groups());
    for (ordinal, spec, nested, expected_unresolved) in [
        (
            0,
            PhonemeSpecification::unknown(),
            BoundedSequence::new(),
            true,
        ),
        (
            6,
            phonemes.tokens()[6].phoneme().clone(),
            BoundedSequence::try_from_iter([phones.tokens()[0].clone()]).unwrap(),
            false,
        ),
        (
            1,
            phonemes.tokens()[1].phoneme().clone(),
            BoundedSequence::try_from_iter([phones.tokens()[0].clone()]).unwrap(),
            false,
        ),
    ] {
        let mut tokens = phonemes.tokens().as_slice().to_vec();
        let token = &tokens[ordinal];
        tokens[ordinal] = SpeechPhonemeToken::new(
            token.confidence().clone(),
            token.features().clone(),
            spec,
            token.provenance().clone(),
            nested,
            token.span().clone(),
        )
        .unwrap();
        let candidate = SpeechPhonemeSequence::new(
            phonemes.basis().clone(),
            BoundedSequence::try_from_iter(tokens).unwrap(),
        )
        .unwrap();
        let refusal = PreparedSpeechCorrespondence::prepare(
            &intent, &candidate, &phones, &text, &alignment, &inventory,
        )
        .err()
        .unwrap();
        if expected_unresolved {
            assert!(matches!(refusal, CorrespondenceRefusal::Unresolved));
        } else {
            assert!(matches!(
                refusal,
                CorrespondenceRefusal::ContradictoryNestedRealization
            ));
        }
    }
}
