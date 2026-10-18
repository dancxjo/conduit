#![cfg(feature = "semantic-bindings")]
#[allow(dead_code)]
#[path = "common/frequency_gesture.rs"]
mod fixture;
use conduit_audio::*;
use conduit_core::Unit;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    correspondence::PreparedSpeechCorrespondence, ipa_inventory::PreparedIpaInventory,
    scoped_energy::*, semantic::*, syllable_structure::PreparedSyllableStructure,
};
use fixture::*;
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "declared normalized reference".into(),
        None,
    )
    .unwrap()
}
fn basis(id: &str, power: bool) -> AudioDecibelBasis {
    let reference = AudioDecibelReference::new(
        1,
        id.into(),
        1,
        provenance(),
        if power {
            AudioDecibelReferenceRole::Power
        } else {
            AudioDecibelReferenceRole::AmplitudeMagnitude
        },
        Unit::One,
    )
    .unwrap();
    AudioDecibelBasis::new(
        if power {
            AudioDecibelConvention::PowerTenLog10
        } else {
            AudioDecibelConvention::AmplitudeTwentyLog10
        },
        reference,
    )
    .unwrap()
}

fn control(scope: SpeechAcousticScope, priority: u16, power: bool) -> SpeechScopedEnergyTarget {
    let ratio = if power {
        AudioTrajectoryQuantity::power(1, 2).unwrap()
    } else {
        AudioTrajectoryQuantity::amplitude(1, 2).unwrap()
    };
    let trajectory = AudioQuantityTrajectory::new(
        anchor(),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        provenance(),
        BoundedSequence::try_from_iter([AudioTrajectorySegment::new(
            time(3),
            AudioTrajectoryInterpolation::Step,
            ratio.clone(),
            ratio,
            time(0),
        )
        .unwrap()])
        .unwrap(),
    )
    .unwrap();
    SpeechScopedEnergyTarget::new(
        basis("same-reference", power),
        AudioScopedControlTrajectory::new(
            "energy".into(),
            if power {
                AudioTrajectoryDomain::Power
            } else {
                AudioTrajectoryDomain::Amplitude
            },
            priority,
            format!("scope/{priority}"),
            trajectory,
        )
        .unwrap(),
        scope,
    )
    .unwrap()
}
fn set(
    targets: Vec<SpeechScopedEnergyTarget>,
    conflict: AudioControlConflictPolicy,
) -> SpeechScopedEnergyGestureSet {
    SpeechScopedEnergyGestureSet::new(
        conflict,
        AudioControlGapPolicy::RefuseAnyGap,
        BoundedSequence::try_from_iter(targets).unwrap(),
    )
    .unwrap()
}
fn supplied_ratio(power: bool, reference: &str, n: u64) -> AudioReferencedLevelRatio {
    AudioReferencedLevelRatio::new(
        basis(reference, power),
        if power {
            AudioLevelRatio::power(1, n).unwrap()
        } else {
            AudioLevelRatio::amplitude(1, n).unwrap()
        },
    )
    .unwrap()
}
fn known_error(error: AudioReferencedLevelRatio) -> SpeechEnergyErrorSpecification {
    SpeechEnergyErrorSpecification::known(error.basis().clone(), error.ratio().clone()).unwrap()
}

fn supplied_observation(
    scope: SpeechAcousticScope,
    power: bool,
    interpretation: SpeechEnergyObservationInterpretation,
    error: SpeechEnergyErrorSpecification,
    reference: &str,
) -> SpeechScopedEnergyObservation {
    SpeechScopedEnergyObservation::new(
        error,
        original::confidence(),
        time(1),
        interpretation,
        original::provenance(),
        scope,
        time(0),
        supplied_ratio(power, reference, 1),
    )
    .unwrap()
}

#[test]
fn actual_broad_scopes_retain_energy_reference_under_generic_overlap_policy() {
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

    let query = AudioTrajectoryQuery::new(anchor(), time(1)).unwrap();
    for power in [false, true] {
        let targets = all_scopes()
            .into_iter()
            .enumerate()
            .map(|(index, scope)| control(scope, index as u16, power))
            .collect::<Vec<_>>();
        let supplied = set(
            targets.clone(),
            AudioControlConflictPolicy::HighestPriorityRefuseTie,
        );
        let prepared = prepare_scoped_energy(&original, &structure, &supplied).unwrap();
        assert!(core::ptr::eq(prepared.original(), &original));
        assert!(core::ptr::eq(prepared.structure(), &structure));
        assert_eq!(prepared.evaluate(&query).unwrap().selected_indices(), &[4]);
        assert_eq!(prepared.supplied().targets().len(), 5);
        let error = supplied_ratio(power, "same-reference", 1);
        let errors = [
            known_error(error.clone()),
            SpeechEnergyErrorSpecification::unknown(),
            SpeechEnergyErrorSpecification::unspecified(),
            SpeechEnergyErrorSpecification::not_applicable(),
            SpeechEnergyErrorSpecification::variable(
                BoundedSequence::try_from_iter([
                    error.clone(),
                    supplied_ratio(power, "same-reference", 2),
                ])
                .unwrap(),
            )
            .unwrap(),
            SpeechEnergyErrorSpecification::gradient(original::confidence(), error).unwrap(),
        ];
        for (index, scope) in all_scopes().into_iter().enumerate() {
            for interpretation in [
                SpeechEnergyObservationInterpretation::Measured,
                SpeechEnergyObservationInterpretation::Inferred,
            ] {
                for error in &errors {
                    let observation = supplied_observation(
                        scope.clone(),
                        power,
                        interpretation,
                        error.clone(),
                        "same-reference",
                    );
                    let loaded = SpeechScopedEnergyObservation::decode(
                        &observation.clone().encode().unwrap(),
                    )
                    .unwrap();
                    let admitted = prepared.observe(index, &loaded).unwrap();
                    assert!(core::ptr::eq(admitted.original(), &original));
                    assert_eq!(admitted.supplied(), &observation);
                    assert_eq!(admitted.supplied().interpretation(), &interpretation);
                    assert_eq!(admitted.supplied().absolute_linear_error(), error);
                    assert_eq!(
                        admitted.target(),
                        &prepared.supplied().targets().as_slice()[index]
                    );
                }
            }
        }
        let foreign_reference = supplied_observation(
            all_scopes()[0].clone(),
            power,
            SpeechEnergyObservationInterpretation::Measured,
            SpeechEnergyErrorSpecification::unknown(),
            "foreign-reference",
        );
        assert!(prepared.observe(0, &foreign_reference).is_err());
        let foreign_error = supplied_observation(
            all_scopes()[0].clone(),
            power,
            SpeechEnergyObservationInterpretation::Inferred,
            known_error(supplied_ratio(power, "foreign-error", 1)),
            "same-reference",
        );
        assert!(prepared.observe(0, &foreign_error).is_err());
        assert_eq!(prepared.supplied(), &supplied);
        let refused = set(targets, AudioControlConflictPolicy::RefuseOverlap);
        assert!(prepare_scoped_energy(&original, &structure, &refused)
            .unwrap()
            .evaluate(&query)
            .is_err());
        let mut bytes = supplied.clone().encode().unwrap();
        let loaded = SpeechScopedEnergyGestureSet::decode(&bytes).unwrap();
        assert_eq!(loaded, supplied);
        bytes.clear();
        let mismatch = set(
            vec![
                control(all_scopes()[0].clone(), 1, false),
                control(all_scopes()[1].clone(), 2, true),
            ],
            AudioControlConflictPolicy::HighestPriorityRefuseTie,
        );
        assert!(prepare_scoped_energy(&original, &structure, &mismatch).is_err());
    }
}
