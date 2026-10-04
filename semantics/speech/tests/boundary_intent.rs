#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{boundary_admission::*, duration::prepare_duration_render, semantic::*, *};
fn intent(
    kind: SpeechBoundarySpecification,
    duration: SpeechDurationSpecification,
) -> SpeechPlannedBoundaryIntent {
    let source = LanguageSegmentRef::text(
        LanguageTextSegmentKind::Phrase,
        SpeechLanguageId::new("en".into()).unwrap(),
        ListeningTextRange::new(1, 0).unwrap(),
        LanguageTextRevisionId::new("r".into()).unwrap(),
        LanguageTextId::new("text".into()).unwrap(),
    )
    .unwrap();
    SpeechPlannedBoundaryIntent::new(
        duration,
        kind,
        SpeechEvidenceProvenance::new("fixture".into(), SpeechEvidenceSource::Manual, None)
            .unwrap(),
        BoundedSequence::try_from_iter([source]).unwrap(),
    )
    .unwrap()
}
#[test]
fn native_binding_admits_only_exact_supported_relations() {
    for kind in [
        SpeechBoundaryKind::Phone,
        SpeechBoundaryKind::Syllable,
        SpeechBoundaryKind::Morpheme,
        SpeechBoundaryKind::Word,
        SpeechBoundaryKind::Phrase,
        SpeechBoundaryKind::BreathGroup,
        SpeechBoundaryKind::Turn,
    ] {
        for realization in [
            SpeechFormantBoundary::Word,
            SpeechFormantBoundary::Phrase,
            SpeechFormantBoundary::Turn,
        ] {
            let expected = matches!(
                (kind, realization),
                (SpeechBoundaryKind::Word, SpeechFormantBoundary::Word)
                    | (SpeechBoundaryKind::Phrase, SpeechFormantBoundary::Phrase)
                    | (SpeechBoundaryKind::Turn, SpeechFormantBoundary::Turn)
            );
            let binding = SpeechFormantBoundaryBinding::new(kind, realization);
            assert_eq!(binding.is_ok(), expected);
            if let Ok(binding) = binding {
                assert_eq!(
                    SpeechFormantBoundaryBinding::from_structured(
                        binding.clone().into_structured().unwrap()
                    )
                    .unwrap(),
                    binding
                );
            }
        }
    }
}
#[test]
fn exact_boundary_intent_retains_sources_and_renders_only_silence() {
    for (kind, realization, compact) in [
        (
            SpeechBoundaryKind::Word,
            SpeechFormantBoundary::Word,
            VoiceBoundary::word,
        ),
        (
            SpeechBoundaryKind::Phrase,
            SpeechFormantBoundary::Phrase,
            VoiceBoundary::phrase,
        ),
        (
            SpeechBoundaryKind::Turn,
            SpeechFormantBoundary::Turn,
            VoiceBoundary::turn,
        ),
    ] {
        let source = intent(
            SpeechBoundarySpecification::Known(kind),
            SpeechDurationSpecification::known(80, 1).unwrap(),
        );
        let binding = SpeechFormantBoundaryBinding::new(kind, realization).unwrap();
        let prepared = prepare_boundary(&source, &binding).unwrap();
        assert!(core::ptr::eq(prepared.intent(), &source));
        assert!(core::ptr::eq(prepared.binding(), &binding));
        assert_eq!(prepared.event(), VoiceEvent::boundary(compact));
        assert_eq!(prepared.checked().requested(), &kind);
        let events = [prepared.event()];
        let mut frames = [-1];
        let timed = prepare_duration_render(
            &events,
            core::slice::from_ref(prepared.duration()),
            &mut frames,
        )
        .unwrap();
        let mut renderer = timed.renderer();
        let mut pcm = [123; 128];
        assert_eq!(renderer.render(&mut pcm).unwrap(), 100);
        assert!(pcm[..100].iter().all(|v| *v == 0));
        assert!(renderer.is_complete());
        let foreign = SpeechFormantBoundaryBinding::new(
            if kind == SpeechBoundaryKind::Word {
                SpeechBoundaryKind::Turn
            } else {
                SpeechBoundaryKind::Word
            },
            if kind == SpeechBoundaryKind::Word {
                SpeechFormantBoundary::Turn
            } else {
                SpeechFormantBoundary::Word
            },
        )
        .unwrap();
        assert!(matches!(
            prepare_boundary(&source, &foreign),
            Err(BoundaryRefusal::Native(_))
        ));
    }
}
#[test]
fn unresolved_kind_and_duration_states_are_preserved() {
    let binding =
        SpeechFormantBoundaryBinding::new(SpeechBoundaryKind::Word, SpeechFormantBoundary::Word)
            .unwrap();
    let confidence = SpeechConfidence::new(IeeeF32::from_value(0.5)).unwrap();
    let kinds = [
        SpeechBoundarySpecification::unknown(),
        SpeechBoundarySpecification::unspecified(),
        SpeechBoundarySpecification::not_applicable(),
        SpeechBoundarySpecification::variable(
            BoundedSequence::try_from_iter([SpeechBoundaryKind::Word]).unwrap(),
        )
        .unwrap(),
        SpeechBoundarySpecification::gradient(confidence.clone(), SpeechBoundaryKind::Word)
            .unwrap(),
    ];
    for specification in kinds {
        let source = intent(
            specification.clone(),
            SpeechDurationSpecification::known(80, 1).unwrap(),
        );
        match prepare_boundary(&source, &binding) {
            Err(BoundaryRefusal::Kind(actual)) => assert_eq!(actual, specification),
            _ => panic!("kind silently resolved"),
        }
    }
    let durations = [
        SpeechDurationSpecification::unknown(),
        SpeechDurationSpecification::unspecified(),
        SpeechDurationSpecification::not_applicable(),
        SpeechDurationSpecification::variable(
            BoundedSequence::try_from_iter([SpeechExactDuration::new(80, 1).unwrap()]).unwrap(),
        )
        .unwrap(),
        SpeechDurationSpecification::gradient(confidence, SpeechExactDuration::new(80, 1).unwrap())
            .unwrap(),
    ];
    for specification in durations {
        let source = intent(
            SpeechBoundarySpecification::Known(SpeechBoundaryKind::Word),
            specification.clone(),
        );
        match prepare_boundary(&source, &binding) {
            Err(BoundaryRefusal::Duration(actual)) => assert_eq!(actual, specification),
            _ => panic!("duration silently resolved"),
        }
    }
}

#[test]
fn boundary_match_laws_preserve_all_seven_kinds_without_collapsing_them() {
    let kinds = [
        SpeechBoundaryKind::Phone,
        SpeechBoundaryKind::Syllable,
        SpeechBoundaryKind::Morpheme,
        SpeechBoundaryKind::Word,
        SpeechBoundaryKind::Phrase,
        SpeechBoundaryKind::BreathGroup,
        SpeechBoundaryKind::Turn,
    ];
    for requested in kinds {
        for supplied in kinds {
            assert_eq!(
                SpeechBoundaryIntentMatch::new(requested, supplied).is_ok(),
                requested == supplied
            );
        }
    }
}
