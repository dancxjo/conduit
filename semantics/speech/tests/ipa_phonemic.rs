#![cfg(feature = "semantic-bindings")]
use conduit_language::{LanguageId, LanguageVariety, VarietyId};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{ipa_inventory::*, semantic::*};

fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "explicit inventory fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
fn fixture(
    definitions: &[(&str, &str, &[&str])],
) -> (
    SpeechInventory,
    SpeechIpaNotationProfile,
    Vec<SpeechIpaPhonemeDefinitionBinding>,
) {
    let language = LanguageId::new("language/test".into()).unwrap();
    let inventory_id = SpeechInventoryId::new("inventory/test".into()).unwrap();
    let phonemes = definitions.iter().map(|(id, spelling, _)| {
        SpeechPhoneme::new(
            BoundedSequence::new(),
            BoundedSequence::new(),
            None,
            SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
            PhonemeId::new((*id).into()).unwrap(),
            (*spelling).into(),
            BoundedSequence::new(),
            SpeechSegmentStatus::Core,
        )
        .unwrap()
    });
    let inventory = SpeechInventory::new(
        inventory_id.clone(),
        language.clone(),
        BoundedSequence::try_from_iter(phonemes).unwrap(),
        BoundedSequence::new(),
    )
    .unwrap();
    let units = [
        ("a", "a", SpeechIpaUnitKind::Segment),
        ("t", "t", SpeechIpaUnitKind::Segment),
        ("s", "s", SpeechIpaUnitKind::Segment),
        ("affricate", "t͡ʃ", SpeechIpaUnitKind::Segment),
        ("nasal", "ã", SpeechIpaUnitKind::Segment),
        ("length", "ː", SpeechIpaUnitKind::Length),
        ("stress", "ˈ", SpeechIpaUnitKind::PrimaryStress),
        ("secondary", "ˌ", SpeechIpaUnitKind::SecondaryStress),
        ("boundary", ".", SpeechIpaUnitKind::SyllableBoundary),
    ]
    .into_iter()
    .map(|(id, spelling, kind)| {
        SpeechIpaUnitDefinition::new(
            SpeechIpaUnitId::new(id.into()).unwrap(),
            kind,
            provenance(),
            SpeechIpaSpelling::new(spelling.into()).unwrap(),
        )
        .unwrap()
    });
    let profile = SpeechIpaNotationProfile::new(
        BoundedSequence::try_from_iter([SpeechIpaSpellingAlias::new(
            provenance(),
            SpeechIpaSpelling::new("ã".into()).unwrap(),
            SpeechIpaUnitId::new("nasal".into()).unwrap(),
        )
        .unwrap()])
        .unwrap(),
        SpeechIpaNotationProfileId::new("notation/test".into()).unwrap(),
        inventory_id,
        provenance(),
        SpeechSegmentRevisionId::new("revision/test".into()).unwrap(),
        BoundedSequence::try_from_iter(units).unwrap(),
        LanguageVariety::new(VarietyId::new("variety/test".into()).unwrap(), language).unwrap(),
    )
    .unwrap();
    let bindings = definitions
        .iter()
        .map(|(id, _, units)| {
            SpeechIpaPhonemeDefinitionBinding::new(
                PhonemeId::new((*id).into()).unwrap(),
                provenance(),
                BoundedSequence::try_from_iter(
                    units
                        .iter()
                        .map(|id| SpeechIpaUnitId::new((*id).into()).unwrap()),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    (inventory, profile, bindings)
}

#[test]
fn transcription_resolves_declared_contrasts_and_retains_unicode_alias_evidence() {
    let (inventory, profile, bindings) = fixture(&[
        ("vowel/short", "a", &["a"]),
        ("vowel/long", "aː", &["a", "length"]),
        ("affricate", "t͡ʃ", &["affricate"]),
        ("nasal", "ã", &["nasal"]),
    ]);
    let prepared = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[],
        &bindings,
    )
    .unwrap();
    let source = "ˈt͡ʃaː.ã";
    let admitted = prepared
        .phonemic_from_ipa(source.into(), provenance())
        .unwrap();
    let value = admitted.transcription();
    assert_eq!(admitted.inventory(), &inventory);
    assert_eq!(value.original(), source);
    assert_eq!(value.notation().original(), "/ˈt͡ʃaː.ã/");
    assert_eq!(value.basis().profile(), &profile);
    assert_eq!(value.bindings().as_slice(), bindings.as_slice());
    assert_eq!(
        SpeechPhonemicTranscription::decode(&value.clone().encode().unwrap()).unwrap(),
        *value
    );
    let mut resolved = Vec::new();
    let mut byte_cursor = 0;
    let mut unit_cursor = 0;
    for occurrence in value.occurrences().as_slice() {
        assert_eq!(*occurrence.start() as usize, byte_cursor);
        assert_eq!(*occurrence.unit_start() as usize, unit_cursor);
        byte_cursor = *occurrence.end() as usize;
        unit_cursor = *occurrence.unit_end() as usize;
        assert_eq!(
            &source[*occurrence.start() as usize..byte_cursor],
            occurrence.source_spelling().get()
        );
        assert_eq!(
            *occurrence.scalar_start() as usize,
            source[..*occurrence.start() as usize].chars().count()
        );
        assert_eq!(
            *occurrence.scalar_end() as usize,
            source[..byte_cursor].chars().count()
        );
        if let SpeechPhonemicEvent::Phoneme(segment) = occurrence.value() {
            resolved.push(segment.phoneme().get().as_str());
        }
    }
    assert_eq!(byte_cursor, source.len());
    assert_eq!(unit_cursor, value.notation().units().len());
    assert_eq!(resolved, ["affricate", "vowel/long", "nasal"]);
    assert_eq!(value.occurrences()[4].source_spelling().get(), "ã");
    admitted.require_inventory(&inventory).unwrap();
    let changed = SpeechInventory::new(
        inventory.identity().clone(),
        inventory.language().clone(),
        BoundedSequence::new(),
        inventory.phones().clone(),
    )
    .unwrap();
    assert!(matches!(
        admitted.require_inventory(&changed),
        Err(IpaInventoryRefusal::Basis)
    ));
    let foreign_revision = SpeechSegmentRevisionId::new("foreign/revision".into()).unwrap();
    let foreign_profile = SpeechIpaNotationProfile::new(
        profile.aliases().clone(),
        profile.identity().clone(),
        profile.inventory_id().clone(),
        profile.provenance().clone(),
        foreign_revision.clone(),
        profile.units().clone(),
        profile.variety().clone(),
    )
    .unwrap();
    let foreign_basis = SpeechIpaInventoryNotationBasis::new(
        inventory.identity().clone(),
        inventory.language().clone(),
        foreign_profile,
        foreign_revision,
        profile.variety().clone(),
    )
    .unwrap();
    assert!(SpeechPhonemicTranscription::new(
        foreign_basis,
        value.bindings().clone(),
        value.notation().clone(),
        value.occurrences().clone(),
        value.original().clone(),
    )
    .is_err());
}

#[test]
fn complete_partition_refuses_competing_definitions_and_groupings() {
    for definitions in [
        vec![("one", "a", &["a"][..]), ("two", "a", &["a"][..])],
        vec![
            ("t", "t", &["t"][..]),
            ("s", "s", &["s"][..]),
            ("ts", "ts", &["t", "s"][..]),
        ],
    ] {
        let (inventory, profile, bindings) = fixture(&definitions);
        let prepared = PreparedIpaInventory::prepare(
            &inventory,
            &profile,
            profile.variety(),
            profile.revision(),
            &[],
            &bindings,
        )
        .unwrap();
        let source = if definitions[0].1 == "a" {
            "ˈa"
        } else {
            "ˈts"
        };
        let error = match prepared.phonemic_from_ipa(source.into(), provenance()) {
            Ok(_) => panic!("ambiguous phonemes"),
            Err(error) => error,
        };
        assert!(matches!(
            error.refusal,
            IpaInventoryRefusal::AmbiguousPhoneme
        ));
        assert_eq!(
            (error.span.byte_start, error.span.byte_end),
            ("ˈ".len(), source.len())
        );
    }
}

#[test]
fn unbound_length_and_known_ipa_outside_the_inventory_refuse_precisely() {
    let (inventory, profile, bindings) = fixture(&[("a", "a", &["a"])]);
    let prepared = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[],
        &bindings,
    )
    .unwrap();
    for (source, unknown) in [("aː", "ː"), ("aã", "ã"), ("at͡ʃ", "t͡ʃ")] {
        let error = match prepared.phonemic_from_ipa(source.into(), provenance()) {
            Ok(_) => panic!("undeclared contrast"),
            Err(error) => error,
        };
        assert!(matches!(error.refusal, IpaInventoryRefusal::UnknownPhoneme));
        assert_eq!(&source[error.span.byte_start..error.span.byte_end], unknown);
        assert_eq!(error.span.scalar_start, 1);
    }
    let error = match prepared.phonemic_from_ipa("a#".into(), provenance()) {
        Ok(_) => panic!("unsupported syntax"),
        Err(error) => error,
    };
    assert!(matches!(error.refusal, IpaInventoryRefusal::Notation(_)));
    assert_eq!((error.span.byte_start, error.span.byte_end), (1, 2));
}

#[test]
fn version_one_refuses_stress_inside_a_phoneme_binding() {
    for (spelling, units) in [
        ("ˈa", &["stress", "a"][..]),
        ("a.a", &["a", "boundary", "a"][..]),
    ] {
        let (inventory, profile, bindings) = fixture(&[("marked", spelling, units)]);
        assert!(matches!(
            PreparedIpaInventory::prepare(
                &inventory,
                &profile,
                profile.variety(),
                profile.revision(),
                &[],
                &bindings,
            ),
            Err(IpaInventoryRefusal::UnsupportedPhonemeBinding)
        ));
    }
}

#[test]
fn capacity_refusals_use_body_coordinates_before_display_copying() {
    use conduit_speech::ipa_notation::IpaNotationRefusal;
    let (inventory, profile, bindings) = fixture(&[("a", "a", &["a"])]);
    let prepared = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[],
        &bindings,
    )
    .unwrap();
    for (source, bytes, scalars) in [
        ("a".repeat(257), (256, 257), (256, 257)),
        ("ã".repeat(3000), (4094, 4096), (2047, 2048)),
    ] {
        let error = match prepared.phonemic_from_ipa(source, provenance()) {
            Ok(_) => panic!("capacity"),
            Err(error) => error,
        };
        assert!(matches!(
            error.refusal,
            IpaInventoryRefusal::Notation(IpaNotationRefusal::Capacity)
        ));
        assert_eq!((error.span.byte_start, error.span.byte_end), bytes);
        assert_eq!((error.span.scalar_start, error.span.scalar_end), scalars);
    }
    assert!(matches!(
        prepared.phoneme_from_ipa("a".repeat(65), provenance()),
        Err(IpaInventoryRefusal::Notation(IpaNotationRefusal::Capacity))
    ));
}
