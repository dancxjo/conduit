#![cfg(feature = "semantic-bindings")]
#[path = "common/frequency_gesture.rs"]
mod fixture;
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    correspondence::PreparedSpeechCorrespondence, intent_frequency_trajectory::*,
    ipa_inventory::PreparedIpaInventory, semantic::*,
    syllable_structure::PreparedSyllableStructure,
};
use fixture::*;

#[test]
fn all_original_scopes_overlap_without_phone_boxes_and_observations_remain_distinct() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        original::inventory(),
        original::profile(),
        original::text(),
        original::intent(),
        original::phonemes(),
        original::phones(),
    );
    let (pb, fb) = original::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[pb],
        &[fb],
    )
    .unwrap();
    let alignment = original::alignment(original::groups());
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let sources = sources();
    let boundaries = [boundary()];
    let structure =
        PreparedSyllableStructure::prepare(&intent, &phones, &text, &sources, &boundaries).unwrap();
    let before = intent.clone().encode().unwrap();
    for scope in all_scopes() {
        let supplied = set(
            vec![target(
                scope.clone(),
                SpeechFrequencyRole::Fundamental,
                "F0",
                "scope/a",
                1,
                120,
                (0, 3),
            )],
            AudioControlConflictPolicy::RefuseOverlap,
        );
        let supplied =
            SpeechFrequencyGestureSet::decode(&supplied.clone().encode().unwrap()).unwrap();
        let prepared = prepare_frequency_gestures(&original, &structure, &supplied).unwrap();
        assert!(core::ptr::eq(prepared.original(), &original));
        let query = AudioTrajectoryQuery::new(anchor(), time(1)).unwrap();
        let converted = prepared.fundamental_cycle(0, &query).unwrap();
        assert_eq!(
            converted.conversion().result(),
            &AudioCycleDuration::new(120, 1).unwrap()
        );
        assert!(!converted.conversion().source_program_hex().is_empty());
        let positive_error =
            SpeechFrequencyErrorAllowance::positive(AudioFrequencyHz::new(1, 2).unwrap()).unwrap();
        let states = [
            SpeechFrequencyErrorSpecification::known(SpeechFrequencyErrorAllowance::Exact).unwrap(),
            SpeechFrequencyErrorSpecification::unknown(),
            SpeechFrequencyErrorSpecification::unspecified(),
            SpeechFrequencyErrorSpecification::not_applicable(),
            SpeechFrequencyErrorSpecification::variable(
                BoundedSequence::try_from_iter([
                    SpeechFrequencyErrorAllowance::Exact,
                    positive_error.clone(),
                ])
                .unwrap(),
            )
            .unwrap(),
            SpeechFrequencyErrorSpecification::gradient(original::confidence(), positive_error)
                .unwrap(),
        ];
        for state in states {
            let evidence = observation(
                scope.clone(),
                SpeechFrequencyRole::Fundamental,
                state.clone(),
            );
            for observed in [
                SpeechFrequencyObservation::measured(evidence.clone()).unwrap(),
                SpeechFrequencyObservation::inferred(evidence.clone()).unwrap(),
            ] {
                let admitted = prepared.observe(0, &observed).unwrap();
                assert_eq!(admitted.supplied(), &observed);
                assert_eq!(admitted.evidence().provenance(), &original::provenance());
                assert_eq!(admitted.evidence().span(), &time(1));
                assert_eq!(admitted.evidence().confidence(), &original::confidence());
                assert_eq!(admitted.evidence().error(), &state);
                assert_eq!(
                    admitted.fundamental_cycle().unwrap().result(),
                    &AudioCycleDuration::new(110, 1).unwrap()
                );
                assert_eq!(prepared.targets(), &supplied);
            }
        }
    }
    assert_eq!(intent.clone().encode().unwrap(), before);
}
#[test]
fn generic_overlap_priority_gap_and_wrong_role_refusals_survive_linguistic_binding() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        original::inventory(),
        original::profile(),
        original::text(),
        original::intent(),
        original::phonemes(),
        original::phones(),
    );
    let (pb, fb) = original::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[pb],
        &[fb],
    )
    .unwrap();
    let alignment = original::alignment(original::groups());
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let sources = sources();
    let boundaries = [boundary()];
    let structure =
        PreparedSyllableStructure::prepare(&intent, &phones, &text, &sources, &boundaries).unwrap();
    let scopes = all_scopes();
    let targets = vec![
        target(
            scopes[0].clone(),
            SpeechFrequencyRole::Fundamental,
            "F0",
            "phone",
            1,
            120,
            (0, 3),
        ),
        target(
            scopes[2].clone(),
            SpeechFrequencyRole::Fundamental,
            "F0",
            "word",
            2,
            140,
            (0, 3),
        ),
        target(
            scopes[3].clone(),
            SpeechFrequencyRole::FormantCenter,
            "F1",
            "phrase",
            0,
            500,
            (0, 3),
        ),
        target(
            scopes[4].clone(),
            SpeechFrequencyRole::Bandwidth,
            "F1-width",
            "utterance",
            0,
            50,
            (0, 3),
        ),
    ];
    let priority = prepare_frequency_gestures(
        &original,
        &structure,
        &set(
            targets.clone(),
            AudioControlConflictPolicy::HighestPriorityRefuseTie,
        ),
    )
    .unwrap();
    let query = AudioTrajectoryQuery::new(anchor(), time(1)).unwrap();
    let result = priority.evaluate(&query).unwrap();
    assert_eq!(result.selected_indices(), &[1, 2, 3]);
    assert_eq!(result.evaluations().len(), 4);
    assert!(matches!(
        priority.fundamental_cycle(0, &query),
        Err(FrequencyGestureRefusal::UnselectedTarget { index: 0 })
    ));
    assert!(matches!(
        priority.fundamental_cycle(2, &query),
        Err(FrequencyGestureRefusal::Role)
    ));
    assert_eq!(
        priority
            .fundamental_cycle(1, &query)
            .unwrap()
            .conversion()
            .result(),
        &AudioCycleDuration::new(140, 1).unwrap()
    );
    let refuse = prepare_frequency_gestures(
        &original,
        &structure,
        &set(targets, AudioControlConflictPolicy::RefuseOverlap),
    )
    .unwrap();
    assert!(matches!(
        refuse.evaluate(&query),
        Err(FrequencyGestureRefusal::Audio(
            AudioControlRefusal::Conflict { .. }
        ))
    ));
    assert!(priority
        .evaluate(&AudioTrajectoryQuery::new(anchor(), time(4)).unwrap())
        .is_err());
    let gapped = prepare_frequency_gestures(
        &original,
        &structure,
        &set(
            vec![gapped_target(scopes[0].clone())],
            AudioControlConflictPolicy::RefuseOverlap,
        ),
    )
    .unwrap();
    let gap_query =
        AudioTrajectoryQuery::new(anchor(), AudioExactTimeOffset::new(2, 3).unwrap()).unwrap();
    assert!(matches!(
        gapped.evaluate(&gap_query),
        Err(FrequencyGestureRefusal::Audio(
            AudioControlRefusal::Trajectory {
                refusal: AudioTrajectoryRefusal::MissingCoverage,
                ..
            }
        ))
    ));
    let mixed_roles = set(
        vec![
            target(
                scopes[0].clone(),
                SpeechFrequencyRole::Fundamental,
                "shared",
                "one",
                1,
                120,
                (0, 3),
            ),
            target(
                scopes[1].clone(),
                SpeechFrequencyRole::Bandwidth,
                "shared",
                "two",
                2,
                50,
                (0, 3),
            ),
        ],
        AudioControlConflictPolicy::HighestPriorityRefuseTie,
    );
    assert!(prepare_frequency_gestures(&original, &structure, &mixed_roles).is_err());
    let wrong = SpeechFrequencyObservation::measured(observation(
        scopes[2].clone(),
        SpeechFrequencyRole::Bandwidth,
        SpeechFrequencyErrorSpecification::unknown(),
    ))
    .unwrap();
    assert!(priority.observe(1, &wrong).is_err());
    let missing = SpeechAcousticScope::syllable(
        SpeechAcousticSyllableScope::new(
            carrier(),
            SpeechSyllableId::new("absent".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_frequency_gestures(
            &original,
            &structure,
            &set(
                vec![target(
                    missing,
                    SpeechFrequencyRole::Fundamental,
                    "F0",
                    "absent",
                    0,
                    120,
                    (0, 3)
                )],
                AudioControlConflictPolicy::RefuseOverlap
            )
        ),
        Err(FrequencyGestureRefusal::Syllable)
    ));
}

#[test]
fn exact_original_data_foreign_scopes_and_unsupported_interpolation_profiles_refuse() {
    let (inventory, profile, text, intent, phonemes, phones) = (
        original::inventory(),
        original::profile(),
        original::text(),
        original::intent(),
        original::phonemes(),
        original::phones(),
    );
    let (pb, fb) = original::bindings();
    let inventory = PreparedIpaInventory::prepare(
        &inventory,
        &profile,
        profile.variety(),
        profile.revision(),
        &[pb],
        &[fb],
    )
    .unwrap();
    let alignment = original::alignment(original::groups());
    let original = PreparedSpeechCorrespondence::prepare(
        &intent, &phonemes, &phones, &text, &alignment, &inventory,
    )
    .unwrap();
    let sources = sources();
    let boundaries = [boundary()];
    let structure =
        PreparedSyllableStructure::prepare(&intent, &phones, &text, &sources, &boundaries).unwrap();
    let valid = set(
        vec![target(
            all_scopes()[0].clone(),
            SpeechFrequencyRole::Fundamental,
            "F0",
            "phone",
            0,
            120,
            (0, 3),
        )],
        AudioControlConflictPolicy::RefuseOverlap,
    );
    let copied_intent = intent.clone();
    let copied_structure =
        PreparedSyllableStructure::prepare(&copied_intent, &phones, &text, &sources, &boundaries)
            .unwrap();
    assert!(matches!(
        prepare_frequency_gestures(&original, &copied_structure, &valid),
        Err(FrequencyGestureRefusal::ForeignStructure)
    ));
    let stale_source = LanguageTextSegmentRef::new(
        *original::source().kind(),
        original::source().language().clone(),
        original::source().range().clone(),
        LanguageTextRevisionId::new("stale".into()).unwrap(),
        original::source().text_id().clone(),
    )
    .unwrap();
    let stale = SpeechAcousticScope::utterance(
        SpeechAcousticCarrierBasis::new(
            anchor(),
            carrier().inventory_basis().clone(),
            stale_source,
        )
        .unwrap(),
    )
    .unwrap();
    let missing_word =
        SpeechAcousticScope::word(SpeechAcousticWordScope::new(carrier(), word(1, 3)).unwrap())
            .unwrap();
    let foreign_phone = SpeechAcousticScope::phone(
        SpeechPhoneAcousticScope::new(
            anchor(),
            carrier().inventory_basis().clone(),
            original::reference("phones", 99),
            original::source(),
        )
        .unwrap(),
    )
    .unwrap();
    let foreign_boundary = SpeechPositionedBoundary::new(
        original::basis("phones"),
        SpeechBoundaryKind::Phrase,
        6,
        original::source(),
    )
    .unwrap();
    let phrase = SpeechAcousticScope::phrase(
        SpeechAcousticPhraseScope::new(
            foreign_boundary,
            carrier(),
            6,
            original::reference("phones", 0),
        )
        .unwrap(),
    )
    .unwrap();
    for scope in [stale, missing_word, foreign_phone, phrase] {
        let invalid = set(
            vec![target(
                scope,
                SpeechFrequencyRole::Fundamental,
                "F0",
                "foreign",
                0,
                120,
                (0, 3),
            )],
            AudioControlConflictPolicy::RefuseOverlap,
        );
        assert!(prepare_frequency_gestures(&original, &structure, &invalid).is_err());
    }
    // Same inventory name/language, but a foreign checked inventory revision.
    let old = original::profile();
    let revision = SpeechSegmentRevisionId::new("inventory/foreign".into()).unwrap();
    let foreign_profile = SpeechIpaNotationProfile::new(
        old.aliases().clone(),
        old.identity().clone(),
        old.inventory_id().clone(),
        old.provenance().clone(),
        revision.clone(),
        old.units().clone(),
        old.variety().clone(),
    )
    .unwrap();
    let foreign_basis = SpeechIpaInventoryNotationBasis::new(
        carrier().inventory_basis().inventory_id().clone(),
        carrier().inventory_basis().language().clone(),
        foreign_profile,
        revision,
        old.variety().clone(),
    )
    .unwrap();
    let scope = SpeechAcousticScope::utterance(
        SpeechAcousticCarrierBasis::new(anchor(), foreign_basis, original::source()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_frequency_gestures(
            &original,
            &structure,
            &set(
                vec![target(
                    scope,
                    SpeechFrequencyRole::Fundamental,
                    "F0",
                    "stale-basis",
                    0,
                    120,
                    (0, 3)
                )],
                AudioControlConflictPolicy::RefuseOverlap
            )
        ),
        Err(FrequencyGestureRefusal::InventoryBasis)
    ));
    let base = target(
        all_scopes()[0].clone(),
        SpeechFrequencyRole::Fundamental,
        "F0",
        "linear",
        0,
        120,
        (0, 3),
    );
    let trajectory = AudioQuantityTrajectory::new(
        anchor(),
        *base.control().trajectory().endpoints(),
        *base.control().trajectory().outside(),
        base.control().trajectory().provenance().clone(),
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            time(u64::MAX),
            AudioTrajectoryInterpolation::Linear,
            AudioTrajectoryQuantity::frequency(1, 120).unwrap(),
            AudioTrajectoryQuantity::frequency(1, 140).unwrap(),
            time(0),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    let control = AudioScopedControlTrajectory::new(
        "F0".into(),
        AudioTrajectoryDomain::Frequency,
        0,
        "linear".into(),
        trajectory,
    )
    .unwrap();
    let unsupported = SpeechFrequencyGestureTarget::new(
        control,
        SpeechFrequencyRole::Fundamental,
        all_scopes()[0].clone(),
    )
    .unwrap();
    assert!(matches!(
        prepare_frequency_gestures(
            &original,
            &structure,
            &set(vec![unsupported], AudioControlConflictPolicy::RefuseOverlap)
        ),
        Err(FrequencyGestureRefusal::Audio(_))
    ));
}
