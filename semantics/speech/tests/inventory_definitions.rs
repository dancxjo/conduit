#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};
use conduit_speech::{inventory_admission::*, reference_admission::*, semantic::*};

fn material(spec: PhoneSpecification) -> (LanguageSegmentRef, SpeechPhoneSequence) {
    let inventory = SpeechInventoryId::new("inventory".into()).unwrap();
    let language = SpeechLanguageId::new("en".into()).unwrap();
    let revision = SpeechSegmentRevisionId::new("revision".into()).unwrap();
    let sequence = SpeechSegmentSequenceId::new("sequence".into()).unwrap();
    let utterance = SpeechUtteranceId::new("utterance".into()).unwrap();
    let reference = LanguageSegmentRef::phone(
        inventory.clone(),
        language.clone(),
        0,
        revision.clone(),
        sequence.clone(),
        utterance.clone(),
    )
    .unwrap();
    let basis =
        SpeechTokenSequenceBasis::new(inventory, language, revision, sequence, utterance).unwrap();
    let token = SpeechPhoneToken::new(
        BoundedSequence::new(),
        SpeechConfidence::new(IeeeF32::from_value(0.75)).unwrap(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        spec,
        SpeechEvidenceProvenance::new("original".into(), SpeechEvidenceSource::Manual, None)
            .unwrap(),
        None,
    )
    .unwrap();
    (
        reference,
        SpeechPhoneSequence::new(basis, BoundedSequence::try_from_iter([token]).unwrap()).unwrap(),
    )
}
fn id(s: &str) -> PhoneId {
    PhoneId::new(s.into()).unwrap()
}
fn definition(identity: &str) -> SpeechPhone {
    SpeechPhone::new(
        BoundedSequence::new(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        id(identity),
        "t".into(),
        SpeechSegmentStatus::Allophonic,
    )
    .unwrap()
}
fn inventory(identity: &str, language: &str, phones: Vec<SpeechPhone>) -> SpeechInventory {
    SpeechInventory::new(
        SpeechInventoryId::new(identity.into()).unwrap(),
        SpeechLanguageId::new(language.into()).unwrap(),
        BoundedSequence::new(),
        BoundedSequence::try_from_iter(phones).unwrap(),
    )
    .unwrap()
}
#[test]
fn definition_resolution_retains_the_exact_material_and_inventory() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let resolved = resolve_phone(&reference, &snapshot).unwrap();
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    let found = resolve_inventory_phone(&resolved, &inventory).unwrap();
    assert!(core::ptr::eq(found.material(), &resolved));
    assert!(core::ptr::eq(found.inventory(), &inventory));
    assert!(core::ptr::eq(
        found.definition(),
        &inventory.phones().as_slice()[0]
    ));
    assert_eq!(
        found.definition().status(),
        &SpeechSegmentStatus::Allophonic
    );
    assert_eq!(found.checked_basis().basis(), snapshot.basis());
    assert_eq!(
        found.checked_identity().requested(),
        found.definition().identity()
    );
    assert_eq!(found.material().token(), &snapshot.tokens().as_slice()[0]);
}
#[test]
fn inventory_and_language_mismatch_refuse_before_lookup() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let resolved = resolve_phone(&reference, &snapshot).unwrap();
    for (identity, language) in [("other", "en"), ("inventory", "es")] {
        let inventory = inventory(identity, language, vec![definition("opaque/t")]);
        assert!(matches!(
            resolve_inventory_phone(&resolved, &inventory),
            Err(InventoryRefusal::Basis(
                NativeBindingRefusal::ViolatedInvariant { .. }
            ))
        ));
    }
}
#[test]
fn same_ipa_does_not_substitute_for_identity_and_duplicates_are_ambiguous() {
    let (reference, snapshot) = material(PhoneSpecification::known(id("opaque/t")).unwrap());
    let resolved = resolve_phone(&reference, &snapshot).unwrap();
    let absent = inventory("inventory", "en", vec![definition("other/t")]);
    assert!(matches!(
        resolve_inventory_phone(&resolved, &absent),
        Err(InventoryRefusal::MissingDefinition)
    ));
    let duplicate = inventory(
        "inventory",
        "en",
        vec![definition("opaque/t"), definition("opaque/t")],
    );
    assert!(matches!(
        resolve_inventory_phone(&resolved, &duplicate),
        Err(InventoryRefusal::AmbiguousDefinition)
    ));
}
#[test]
fn unresolved_specs_are_returned_exactly_without_a_default_or_selection() {
    let states = [
        PhoneSpecification::unknown(),
        PhoneSpecification::unspecified(),
        PhoneSpecification::not_applicable(),
        PhoneSpecification::variable(BoundedSequence::try_from_iter([id("opaque/t")]).unwrap())
            .unwrap(),
        PhoneSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.25)).unwrap(),
            id("opaque/t"),
        )
        .unwrap(),
    ];
    let inventory = inventory("inventory", "en", vec![definition("opaque/t")]);
    for state in states {
        let (reference, snapshot) = material(state.clone());
        let resolved = resolve_phone(&reference, &snapshot).unwrap();
        match resolve_inventory_phone(&resolved, &inventory) {
            Err(InventoryRefusal::Unresolved(actual)) => assert_eq!(actual, state),
            _ => panic!("unresolved specification must remain unresolved"),
        }
        assert_eq!(resolved.token().phone(), &state);
    }
}

#[test]
fn native_definition_receipt_refuses_an_unrelated_identity() {
    assert!(matches!(
        SpeechPhoneDefinitionMatch::new(id("other"), id("opaque/t")),
        Err(NativeBindingRefusal::ViolatedInvariant { .. })
    ));
}
