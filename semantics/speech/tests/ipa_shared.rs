#![cfg(feature = "semantic-bindings")]

use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{ipa_inventory::*, ipa_shared::*, semantic::*, shared_intent::*};
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

fn sequence_basis(revision: &str) -> SpeechTokenSequenceBasis {
    SpeechTokenSequenceBasis::new(
        SpeechInventoryId::new("inventory/a".into()).unwrap(),
        LanguageId::new("language/test".into()).unwrap(),
        SpeechSegmentRevisionId::new(revision.into()).unwrap(),
        SpeechSegmentSequenceId::new("sequence".into()).unwrap(),
        SpeechUtteranceId::new("utterance".into()).unwrap(),
    )
    .unwrap()
}
fn intent(revision: &str) -> SpeechUtteranceIntent {
    let b = sequence_basis(revision);
    SpeechUtteranceIntent::new(
        BoundedSequence::new(),
        b.inventory_id().clone(),
        b.language().clone(),
        provenance("original intent"),
        b.revision_id().clone(),
        b.utterance_id().clone(),
    )
    .unwrap()
}
fn context(variety: &str) -> SpeechUtteranceIntentContext {
    SpeechUtteranceIntentContext::new(
        None,
        provenance("context"),
        SpeechReferenceProjectionCapability::new("retained_metadata_only".into()).unwrap(),
        SpeechSpeakerReferenceSpecification::unknown(),
        SpeechStyleReferenceSpecification::unspecified(),
        LanguageVariety::new(
            VarietyId::new(variety.into()).unwrap(),
            LanguageId::new("language/test".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
fn shared<'a>(
    i: &'a SpeechUtteranceIntent,
    c: &'a SpeechUtteranceIntentContext,
    p: &'a SpeechPhonemeSequence,
    q: &'a SpeechPhoneSequence,
) -> PreparedSpeechUtteranceIntent<'a> {
    PreparedSpeechUtteranceIntent::prepare(
        i,
        SpeechIntentComponents {
            context: c,
            intended_text: None,
            phonemes: p,
            phones: q,
            morphemes: &[],
            morpheme_texts: &[],
            syllables: &[],
            correspondences: &[],
        },
    )
    .unwrap()
}
fn sequences(
    revision: &str,
    phone: PhoneSpecification,
    phoneme: PhonemeSpecification,
) -> (SpeechPhonemeSequence, SpeechPhoneSequence) {
    let confidence = SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap();
    let q = SpeechPhoneToken::new(
        BoundedSequence::new(),
        confidence,
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        phone,
        provenance("phone"),
        None,
    )
    .unwrap();
    let p = SpeechPhonemeToken::new(
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        phoneme,
        provenance("phoneme"),
        BoundedSequence::new(),
        None,
    )
    .unwrap();
    (
        SpeechPhonemeSequence::new(
            sequence_basis(revision),
            BoundedSequence::try_from_iter([p]).unwrap(),
        )
        .unwrap(),
        SpeechPhoneSequence::new(
            sequence_basis(revision),
            BoundedSequence::try_from_iter([q]).unwrap(),
        )
        .unwrap(),
    )
}
#[test]
fn executed_inventory_join_preserves_original_owners_and_all_spec_states() {
    let inv = inventory("ə");
    let notation = profile("inventory/a", "variety/a", "revision/a", "original");
    let (pb, qb) = bindings();
    let ipa = PreparedIpaInventory::prepare(
        &inv,
        &notation,
        notation.variety(),
        notation.revision(),
        core::slice::from_ref(&pb),
        core::slice::from_ref(&qb),
    )
    .unwrap();
    let i = intent("revision/a");
    let c = context("variety/a");
    let phone = inv.phones()[0].identity().clone();
    let phoneme = inv.phonemes()[0].identity().clone();
    let states = [
        (
            PhoneSpecification::known(phone.clone()).unwrap(),
            PhonemeSpecification::known(phoneme.clone()).unwrap(),
            1,
        ),
        (
            PhoneSpecification::variable(BoundedSequence::try_from_iter([phone.clone()]).unwrap())
                .unwrap(),
            PhonemeSpecification::variable(
                BoundedSequence::try_from_iter([phoneme.clone()]).unwrap(),
            )
            .unwrap(),
            1,
        ),
        (
            PhoneSpecification::gradient(
                SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
                phone,
            )
            .unwrap(),
            PhonemeSpecification::gradient(
                SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
                phoneme,
            )
            .unwrap(),
            1,
        ),
        (
            PhoneSpecification::unknown(),
            PhonemeSpecification::unknown(),
            0,
        ),
        (
            PhoneSpecification::unspecified(),
            PhonemeSpecification::unspecified(),
            0,
        ),
        (
            PhoneSpecification::not_applicable(),
            PhonemeSpecification::not_applicable(),
            0,
        ),
    ];
    for (phone, phoneme, count) in states {
        let (p, q) = sequences("revision/a", phone, phoneme);
        let common = shared(&i, &c, &p, &q);
        SpeechSharedIntentIpaBasis::new(
            c.variety().clone(),
            i.inventory_id().clone(),
            i.language().clone(),
            i.revision_id().clone(),
            ipa.basis().clone(),
        )
        .expect("basis");
        let joined = PreparedIpaSpeechUtteranceIntent::prepare(&common, &ipa).unwrap();
        assert!(core::ptr::eq(joined.shared(), &common));
        assert!(core::ptr::eq(joined.ipa(), &ipa));
        assert!(core::ptr::eq(joined.shared().original(), &i));
        assert_eq!(joined.phones().len(), count);
        assert_eq!(joined.phonemes().len(), count);
        if count == 1 {
            assert!(core::ptr::eq(
                joined.phones()[0].definition(),
                &inv.phones()[0]
            ));
            assert!(core::ptr::eq(
                joined.phonemes()[0].definition(),
                &inv.phonemes()[0]
            ));
            assert_eq!(joined.phones()[0].candidate().token(), &q.tokens()[0]);
            assert_eq!(joined.phonemes()[0].candidate().token(), &p.tokens()[0]);
        }
        assert_eq!(ipa.phones()[0].1.transcription().original(), "[ə]");
        assert_eq!(ipa.phonemes()[0].1.transcription().original(), "/ə/");
    }
}
#[test]
fn foreign_basis_and_missing_actual_inventory_definitions_refuse() {
    let inv = inventory("ə");
    let notation = profile("inventory/a", "variety/a", "revision/a", "original");
    let (pb, qb) = bindings();
    let ipa = PreparedIpaInventory::prepare(
        &inv,
        &notation,
        notation.variety(),
        notation.revision(),
        core::slice::from_ref(&pb),
        core::slice::from_ref(&qb),
    )
    .unwrap();
    for (revision, variety) in [
        ("foreign/revision", "variety/a"),
        ("revision/a", "foreign/variety"),
    ] {
        let i = intent(revision);
        let c = context(variety);
        let (p, q) = sequences(
            revision,
            PhoneSpecification::unknown(),
            PhonemeSpecification::unknown(),
        );
        let common = shared(&i, &c, &p, &q);
        assert!(matches!(
            PreparedIpaSpeechUtteranceIntent::prepare(&common, &ipa),
            Err(SharedIpaRefusal::Native(_))
        ));
    }
    let i = intent("revision/a");
    let c = context("variety/a");
    let (p, q) = sequences(
        "revision/a",
        PhoneSpecification::variable(
            BoundedSequence::try_from_iter([
                inv.phones()[0].identity().clone(),
                PhoneId::new("foreign".into()).unwrap(),
            ])
            .unwrap(),
        )
        .unwrap(),
        PhonemeSpecification::unknown(),
    );
    let common = shared(&i, &c, &p, &q);
    assert!(matches!(
        PreparedIpaSpeechUtteranceIntent::prepare(&common, &ipa),
        Err(SharedIpaRefusal::MissingPhone)
    ));
    let (p, q) = sequences(
        "revision/a",
        PhoneSpecification::unknown(),
        PhonemeSpecification::known(PhonemeId::new("foreign".into()).unwrap()).unwrap(),
    );
    let common = shared(&i, &c, &p, &q);
    assert!(matches!(
        PreparedIpaSpeechUtteranceIntent::prepare(&common, &ipa),
        Err(SharedIpaRefusal::MissingPhoneme)
    ));
}

#[test]
fn external_field_types_match_actual_material() {
    use conduit_plot::rust_binding::record_field_type;
    let ty = SpeechSharedIntentIpaBasis::semantic_type().unwrap();
    for (name, actual) in [
        ("context_variety", LanguageVariety::semantic_type().unwrap()),
        (
            "intent_inventory_id",
            SpeechInventoryId::semantic_type().unwrap(),
        ),
        ("intent_language", LanguageId::semantic_type().unwrap()),
        (
            "intent_revision",
            SpeechSegmentRevisionId::semantic_type().unwrap(),
        ),
        (
            "ipa_basis",
            SpeechIpaInventoryNotationBasis::semantic_type().unwrap(),
        ),
    ] {
        let expected = record_field_type(&ty, name).unwrap();
        println!(
            "{} {} expected{} actual{}",
            name,
            expected == actual,
            expected.canonical_bytes().unwrap().len(),
            actual.canonical_bytes().unwrap().len()
        );
        assert!(
            expected == actual,
            "{} external material Type differs",
            name
        );
    }
}
