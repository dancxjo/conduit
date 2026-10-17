#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
//! Private capacity witness; production gesture meaning remains with its owners.
use conduit_audio::*;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use conduit_speech::semantic::*;
#[path = "common/value_parameter_custody.rs"]
mod custody;
#[allow(dead_code)]
#[path = "common/occurrence_intent.rs"]
mod linguistic;

const SOURCE: &str = "type CapacityEvent = {\n trajectory: Trajectory\n occurrences: sequence Occurrence in 1..=8\n}\n\
type CapacityTier<Maximum: U16> = sequence CapacityEvent <= Maximum\n\
type CapacityWitness<First: U16, Second: U16> = {\n carrier: Intent\n first: CapacityTier<First>\n second: CapacityTier<Second>\n}\n";

fn witness_type(first: u16, second: u16) -> StructuredInfoType {
    let mut catalog = StartupCatalog::new();
    for (name, ty) in [
        ("Intent", SpeechUtteranceIntent::semantic_type().unwrap()),
        (
            "Occurrence",
            LanguageSpeechTokenRef::semantic_type().unwrap(),
        ),
        (
            "Trajectory",
            AudioQuantityTrajectory::semantic_type().unwrap(),
        ),
    ] {
        catalog.insert_structured_type(name, ty).unwrap();
    }
    let source = format!("{SOURCE}type Witness = CapacityWitness<{first}, {second}>\n");
    check_syntax_document(&parse_syntax_document(&source), &catalog)
        .unwrap()
        .native_types
        .into_iter()
        .find(|ty| ty.name == "Witness")
        .unwrap()
        .value_type
}
fn anchor(timeline: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(timeline).unwrap(),
    )
    .unwrap()
}
fn trajectory(start: u64, end: u64, frequency: bool) -> AudioQuantityTrajectory {
    let quantity = if frequency {
        AudioTrajectoryQuantity::frequency(1, 100).unwrap()
    } else {
        AudioTrajectoryQuantity::amplitude(2, 1).unwrap()
    };
    AudioQuantityTrajectory::new(
        anchor(1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "capacity witness; not phonological realization".into(),
            Some("1".into()),
        )
        .unwrap(),
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            AudioExactTimeOffset::new(20, end).unwrap(),
            AudioTrajectoryInterpolation::Step,
            quantity.clone(),
            quantity,
            AudioExactTimeOffset::new(20, start).unwrap(),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap()
}
fn sequence(
    ty: StructuredInfoType,
    values: Vec<StructuredInfoValue>,
) -> Result<StructuredInfoValue, conduit_core::StructuredInfoRefusal> {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            let value = sequence(representation.clone(), values)?;
            StructuredInfoValue::nominal(ty, value)
        }
        StructuredInfoTypeShape::Sequence { .. } => StructuredInfoValue::sequence(ty, values),
        _ => panic!("capacity must remain a variable sequence"),
    }
}
fn event(
    ty: &StructuredInfoType,
    curve: &AudioQuantityTrajectory,
    occurrences: &[u32],
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    let values = fields
        .iter()
        .map(|field| {
            let value = match field.name() {
                "trajectory" => curve.clone().into_structured().unwrap(),
                "occurrences" => sequence(
                    field.value_type().clone(),
                    occurrences
                        .iter()
                        .map(|ordinal| {
                            linguistic::token(*ordinal, linguistic::BASIS)
                                .into_structured()
                                .unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
                _ => panic!(),
            };
            StructuredFieldValue::new(field.name(), value).unwrap()
        })
        .collect();
    StructuredInfoValue::record(ty.clone(), values).unwrap()
}
fn tier(
    ty: StructuredInfoType,
    curves: &[(AudioQuantityTrajectory, Vec<u32>)],
) -> Result<StructuredInfoValue, conduit_core::StructuredInfoRefusal> {
    let mut representation = ty.clone();
    while let StructuredInfoTypeShape::Nominal {
        representation: inner,
        ..
    } = representation.shape()
    {
        representation = inner.clone();
    }
    let StructuredInfoTypeShape::Sequence { element, .. } = representation.shape() else {
        panic!()
    };
    let values = curves
        .iter()
        .map(|(curve, refs)| event(element, curve, refs))
        .collect();
    sequence(ty, values)
}
fn witness(
    ty: StructuredInfoType,
    intent: &SpeechUtteranceIntent,
    first: &[(AudioQuantityTrajectory, Vec<u32>)],
    second: &[(AudioQuantityTrajectory, Vec<u32>)],
) -> Result<StructuredInfoValue, conduit_core::StructuredInfoRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!()
    };
    let values = fields
        .iter()
        .map(|field| {
            let value = match field.name() {
                "carrier" => intent.clone().into_structured().unwrap(),
                "first" => tier(field.value_type().clone(), first)?,
                "second" => tier(field.value_type().clone(), second)?,
                _ => panic!(),
            };
            StructuredFieldValue::new(field.name(), value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::record(ty, values)
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!()
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}
fn count(value: &StructuredInfoValue) -> usize {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        panic!()
    };
    values.len()
}

#[test]
fn independent_variable_tiers_preserve_carrier_relations_and_do_not_pad_capacity() {
    let intent = linguistic::intent([linguistic::segment(0), linguistic::segment(1)]);
    let original = intent.clone().encode().unwrap();
    // One trajectory corresponds to two occurrences; occurrence zero has
    // multiple trajectories. No parallel phone/gesture indexing is inferred.
    let first = vec![
        (trajectory(0, 10, false), vec![0, 1]),
        (trajectory(12, 20, false), vec![0]),
    ];
    let second = vec![
        (trajectory(2, 8, true), vec![0]),
        (trajectory(7, 14, true), vec![0, 1]),
        (trajectory(13, 21, true), vec![1]),
    ];
    for (a, b) in [(3, 7), (5, 9)] {
        let value = witness(witness_type(a, b), &intent, &first, &second).unwrap();
        let decoded =
            StructuredInfoValue::from_canonical_bytes(&value.canonical_bytes().unwrap()).unwrap();
        assert_eq!(count(field(&decoded, "first")), 2);
        assert_eq!(count(field(&decoded, "second")), 3);
        assert_eq!(
            field(&decoded, "carrier").canonical_bytes().unwrap(),
            original
        );
        assert_eq!(
            SpeechUtteranceIntent::decode(&field(&decoded, "carrier").canonical_bytes().unwrap())
                .unwrap(),
            intent
        );
        let StructuredInfoValueShape::Collection(events) = field(&decoded, "first").shape() else {
            panic!()
        };
        assert_eq!(count(field(&events[0], "occurrences")), 2);
        assert_eq!(
            field(&events[0], "trajectory").canonical_bytes().unwrap(),
            first[0].0.clone().encode().unwrap()
        );
        for (name, supplied) in [("first", &first), ("second", &second)] {
            let StructuredInfoValueShape::Collection(events) = field(&decoded, name).shape() else {
                panic!()
            };
            for (event, (curve, ordinals)) in events.iter().zip(supplied.iter()) {
                assert_eq!(
                    field(event, "trajectory").canonical_bytes().unwrap(),
                    curve.clone().encode().unwrap()
                );
                let StructuredInfoValueShape::Collection(refs) =
                    field(event, "occurrences").shape()
                else {
                    panic!()
                };
                assert_eq!(refs.len(), ordinals.len());
                for (reference, ordinal) in refs.iter().zip(ordinals) {
                    let reference =
                        LanguageSpeechTokenRef::decode(&reference.canonical_bytes().unwrap())
                            .unwrap();
                    assert_eq!(reference, linguistic::token(*ordinal, linguistic::BASIS));
                    SpeechOccurrenceMembership::new(
                        intent.inventory_id().clone(),
                        intent.language().clone(),
                        reference,
                        intent.revision_id().clone(),
                        intent.utterance_id().clone(),
                    )
                    .unwrap();
                }
            }
        }
    }
    assert!(witness(
        witness_type(3, 7),
        &intent,
        &vec![first[0].clone(); 4],
        &second
    )
    .is_err());
    assert!(witness(
        witness_type(3, 7),
        &intent,
        &first,
        &vec![second[0].clone(); 8]
    )
    .is_err());
    let empty = witness(witness_type(3, 7), &intent, &[], &second).unwrap();
    assert_eq!(count(field(&empty, "first")), 0);
    assert_eq!(count(field(&empty, "second")), 3);
}

#[test]
fn shared_temporal_anchor_correlates_overlapping_independent_extents_and_foreign_anchor_refuses() {
    let left = trajectory(0, 10, false);
    let right = trajectory(2, 8, true);
    let query = AudioTrajectoryQuery::new(anchor(1), AudioExactTimeOffset::new(20, 5).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    for original in [left, right] {
        let prepared =
            PreparedAudioQuantityTrajectory::new(&original.clone().encode().unwrap()).unwrap();
        let receipt = prepared.evaluate(&query).unwrap();
        assert_eq!(receipt.original(), &original);
        let foreign =
            AudioTrajectoryQuery::new(anchor(2), AudioExactTimeOffset::new(20, 5).unwrap())
                .unwrap()
                .encode()
                .unwrap();
        assert!(matches!(
            prepared.evaluate(&foreign),
            Err(AudioTrajectoryRefusal::ForeignAnchor)
        ));
    }
}

#[test]
fn changing_capacity_preserves_executed_ipa_inventory_source_revision_and_commitment_custody() {
    use conduit_speech::ipa_inventory::PreparedIpaInventory;
    let inventory = custody::inventory();
    let profile = custody::profile();
    let phone = custody::phone_binding();
    let phoneme = custody::phoneme_binding();
    let notation = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        core::slice::from_ref(&phone),
        core::slice::from_ref(&phoneme),
    )
    .unwrap();
    assert_eq!(notation.phones()[0].1.transcription().original(), "[t͡ʃ]");
    assert_eq!(notation.phonemes()[0].1.transcription().original(), "/t͡ʃ/");
    let prepared =
        conduit_speech::ipa_notation::PreparedIpaNotationProfile::prepare(&profile).unwrap();
    let syllable_notation = conduit_speech::ipa_admission::admit_ipa_transcription(
        &prepared,
        "[t͡ʃ.t͡ʃ]".into(),
        SpeechIpaDisplayKind::Phonetic,
        custody::provenance("explicit syllable notation; no inferred syllable occurrence"),
    )
    .unwrap();
    let original_notation = syllable_notation.checked_match().clone().encode().unwrap();
    let original_source = custody::revision(None, "t͡ʃ");
    conduit_language::validate_text_revision(None, &original_source, 0, 8).unwrap();
    let committed_scalars = 3;
    let next = custody::revision(Some(&original_source), "t͡ʃ");
    conduit_language::validate_text_revision(Some(&original_source), &next, committed_scalars, 8)
        .unwrap();
    let changed = custody::revision(Some(&original_source), "k͡ʃ");
    assert_eq!(
        conduit_language::validate_text_revision(
            Some(&original_source),
            &changed,
            committed_scalars,
            8
        ),
        Err(conduit_language::TextRevisionRefusal::CommittedPrefixChanged)
    );
    let intent = linguistic::intent([linguistic::segment(0), linguistic::segment(1)]);
    conduit_speech::intent_admission::validate_intent_occurrences(&intent).unwrap();
    for index in 0..2 {
        let resolved = conduit_speech::intent_inventory::resolve_intent_inventory_phone(
            &intent, index, &inventory,
        )
        .unwrap();
        assert_eq!(resolved.definition().identity().get(), "phone/t");
        assert_eq!(resolved.definition().ipa(), "t͡ʃ");
    }
    let snapshots = (
        inventory.clone().encode().unwrap(),
        profile.clone().encode().unwrap(),
        original_source.clone().encode().unwrap(),
        notation.phones()[0]
            .1
            .checked_match()
            .clone()
            .encode()
            .unwrap(),
        notation.phonemes()[0]
            .1
            .checked_match()
            .clone()
            .encode()
            .unwrap(),
    );
    let first = vec![(trajectory(0, 10, false), vec![0, 1])];
    let second = vec![
        (trajectory(2, 8, true), vec![0]),
        (trajectory(7, 14, true), vec![1]),
    ];
    for (a, b) in [(3, 7), (5, 9)] {
        let stored = witness(witness_type(a, b), &intent, &first, &second).unwrap();
        let decoded =
            SpeechUtteranceIntent::decode(&field(&stored, "carrier").canonical_bytes().unwrap())
                .unwrap();
        assert_eq!(decoded, intent);
        for event in decoded.events().as_slice() {
            let SpeechUtteranceIntentEvent::Segment(segment) = event else {
                panic!()
            };
            let source = segment.sources().as_slice()[0]
                .clone()
                .into_structured()
                .unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = source.shape() else {
                panic!()
            };
            assert_eq!(tag, "text");
            let reference = conduit_language::LanguageTextSegmentRef::decode(
                &payload.canonical_bytes().unwrap(),
            )
            .unwrap();
            conduit_language::LanguageTextReferenceMatch::new(
                original_source.material().clone(),
                reference.clone(),
                committed_scalars,
            )
            .unwrap();
        }
        assert_eq!(inventory.clone().encode().unwrap(), snapshots.0);
        assert_eq!(profile.clone().encode().unwrap(), snapshots.1);
        assert_eq!(original_source.clone().encode().unwrap(), snapshots.2);
        assert_eq!(
            notation.phones()[0]
                .1
                .checked_match()
                .clone()
                .encode()
                .unwrap(),
            snapshots.3
        );
        assert_eq!(
            notation.phonemes()[0]
                .1
                .checked_match()
                .clone()
                .encode()
                .unwrap(),
            snapshots.4
        );
        assert_eq!(syllable_notation.transcription().original(), "[t͡ʃ.t͡ʃ]");
        assert_eq!(
            syllable_notation.checked_match().clone().encode().unwrap(),
            original_notation
        );
    }
}
