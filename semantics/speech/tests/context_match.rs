#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{context_match::*, semantic::*};
fn states(value: SpeechStress) -> [StressSpecification; 6] {
    [
        StressSpecification::known(value).unwrap(),
        StressSpecification::unknown(),
        StressSpecification::unspecified(),
        StressSpecification::not_applicable(),
        StressSpecification::variable(BoundedSequence::try_from_iter([value]).unwrap()).unwrap(),
        StressSpecification::gradient(
            SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap(),
            value,
        )
        .unwrap(),
    ]
}
#[test]
fn all_specification_pairs_preserve_original_values_and_distinct_decisions() {
    let requirements = states(SpeechStress::Primary);
    for observations in [
        states(SpeechStress::Primary),
        states(SpeechStress::Secondary),
    ] {
        for (r, requirement) in requirements.iter().enumerate() {
            for (o, observation) in observations.iter().enumerate() {
                let compared = compare_stress(requirement, observation).unwrap();
                assert!(core::ptr::eq(compared.requirement(), requirement));
                assert!(core::ptr::eq(compared.observation(), observation));
                let expected = if r == 2 {
                    SpeechContextDecision::Matched
                } else if r != 0 {
                    SpeechContextDecision::RequirementUnresolved
                } else if o != 0 {
                    SpeechContextDecision::ObservationUnresolved
                } else if requirement == observation {
                    SpeechContextDecision::Matched
                } else {
                    SpeechContextDecision::Mismatched
                };
                assert_eq!(compared.decision(), &expected);
            }
        }
    }
}
#[test]
fn all_scalar_environment_domains_compare_exact_known_values() {
    for value in [
        SpeechWordPosition::Initial,
        SpeechWordPosition::Medial,
        SpeechWordPosition::Final,
        SpeechWordPosition::Isolated,
    ] {
        let expected = SpeechPositionSpecification::known(value).unwrap();
        let actual = SpeechPositionSpecification::known(SpeechWordPosition::Final).unwrap();
        let compared = compare_word_position(&expected, &actual).unwrap();
        assert_eq!(
            compared.decision(),
            if value == SpeechWordPosition::Final {
                &SpeechContextDecision::Matched
            } else {
                &SpeechContextDecision::Mismatched
            }
        );
    }
    for value in [
        SpeechSyllablePosition::Onset,
        SpeechSyllablePosition::Nucleus,
        SpeechSyllablePosition::Coda,
    ] {
        let expected = SpeechSyllablePositionSpecification::known(value).unwrap();
        let actual =
            SpeechSyllablePositionSpecification::known(SpeechSyllablePosition::Nucleus).unwrap();
        assert_eq!(
            compare_syllable_position(&expected, &actual)
                .unwrap()
                .decision(),
            if value == SpeechSyllablePosition::Nucleus {
                &SpeechContextDecision::Matched
            } else {
                &SpeechContextDecision::Mismatched
            }
        );
    }
    let careful =
        SpeechProsodicContextSpecification::known(SpeechProsodicContext::CarefulSpeech).unwrap();
    let fast =
        SpeechProsodicContextSpecification::known(SpeechProsodicContext::FastSpeech).unwrap();
    let missing = SpeechProsodicContextSpecification::unknown();
    assert_eq!(
        compare_prosodic_context(&careful, &careful)
            .unwrap()
            .decision(),
        &SpeechContextDecision::Matched
    );
    assert_eq!(
        compare_prosodic_context(&careful, &fast)
            .unwrap()
            .decision(),
        &SpeechContextDecision::Mismatched
    );
    assert_eq!(
        compare_prosodic_context(&careful, &missing)
            .unwrap()
            .decision(),
        &SpeechContextDecision::ObservationUnresolved
    );
}
