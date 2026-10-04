#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{intent_sources::*, semantic::*};
#[allow(dead_code)]
#[path = "common/native_phone.rs"]
mod fixture;
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "source coverage fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
fn intent(groups: Vec<Vec<LanguageSegmentRef>>) -> SpeechUtteranceIntent {
    try_intent(groups).unwrap()
}
fn try_intent(
    groups: Vec<Vec<LanguageSegmentRef>>,
) -> Result<SpeechUtteranceIntent, conduit_plot::rust_binding::NativeBindingRefusal> {
    let events = groups.into_iter().enumerate().map(|(event, sources)| {
        let sources = BoundedSequence::try_from_iter(sources).unwrap();
        if event == 0 {
            SpeechUtteranceIntentEvent::segment(
                LanguageSpeechTokenRef::new(
                    SpeechInventoryId::new("chosen inventory".into()).unwrap(),
                    SpeechLanguageId::new("en".into()).unwrap(),
                    0,
                    SpeechSegmentRevisionId::new("intent revision".into()).unwrap(),
                    SpeechSegmentSequenceId::new("intent sequence".into()).unwrap(),
                    SpeechUtteranceId::new("intent".into()).unwrap(),
                )
                .unwrap(),
                PhoneSpecification::unknown(),
                PhonemeSpecification::unspecified(),
                SpeechSegmentProsodyIntent::new(
                    SpeechDurationSpecification::unknown(),
                    SpeechCycleSpecification::unknown(),
                    SpeechIntensitySpecification::unknown(),
                )
                .unwrap(),
                provenance(),
                sources,
                StressSpecification::unknown(),
                SpeechPositionSpecification::unknown(),
            )
            .unwrap()
        } else {
            SpeechUtteranceIntentEvent::boundary(
                SpeechDurationSpecification::unknown(),
                SpeechBoundarySpecification::unknown(),
                provenance(),
                sources,
            )
            .unwrap()
        }
    });
    SpeechUtteranceIntent::new(
        BoundedSequence::try_from_iter(events).unwrap(),
        SpeechInventoryId::new("chosen inventory".into()).unwrap(),
        SpeechLanguageId::new("en".into()).unwrap(),
        provenance(),
        SpeechSegmentRevisionId::new("intent revision".into()).unwrap(),
        SpeechUtteranceId::new("intent".into()).unwrap(),
    )
}
fn text(revision: &str) -> LanguageText {
    LanguageText::new(
        LanguageTextId::new("source text".into()).unwrap(),
        SpeechLanguageId::new("es".into()).unwrap(),
        LanguageTextRevisionId::new(revision.into()).unwrap(),
        "¡Qué!".into(),
    )
    .unwrap()
}
fn text_ref() -> LanguageSegmentRef {
    LanguageSegmentRef::text(
        LanguageTextSegmentKind::Word,
        SpeechLanguageId::new("es".into()).unwrap(),
        ListeningTextRange::new(4, 1).unwrap(),
        LanguageTextRevisionId::new("source revision".into()).unwrap(),
        LanguageTextId::new("source text".into()).unwrap(),
    )
    .unwrap()
}
fn envelope(stream: &str, event: &str) -> AsrRecognitionEnvelope {
    let time = ListeningEventTime::new(0, ListeningClockOrigin::StreamStart).unwrap();
    AsrRecognitionEnvelope::new(
        AsrRecognitionEvent::Completed,
        ListeningEventId::new(event.into()).unwrap(),
        ListeningProvenance::new(
            BoundedSequence::new(),
            ListeningProvenanceKind::Direct,
            None,
            None,
            BoundedSequence::new(),
        )
        .unwrap(),
        17,
        ListeningStreamId::new(stream.into()).unwrap(),
        ListeningEventTimes::new(time.clone(), time).unwrap(),
    )
    .unwrap()
}
fn recognition_ref() -> LanguageSegmentRef {
    LanguageSegmentRef::recognition(
        ListeningEventId::new("event".into()).unwrap(),
        ListeningStreamId::new("stream".into()).unwrap(),
    )
    .unwrap()
}
#[test]
fn covers_original_segment_and_boundary_sources_without_resolving_intent_specs() {
    let (phone_ref, phones) = fixture::material(PhoneSpecification::unknown());
    let source = intent(vec![
        vec![phone_ref.clone()],
        vec![text_ref(), recognition_ref()],
    ]);
    let text = text("source revision");
    let envelope = envelope("stream", "event");
    let prepared = resolve_intent_sources(
        &source,
        &[
            IntentSourceMaterial::Phone(&phones),
            IntentSourceMaterial::Text(&text),
            IntentSourceMaterial::Recognition(&envelope),
        ],
    )
    .unwrap();
    assert!(core::ptr::eq(prepared.intent(), &source));
    assert_eq!(
        prepared
            .receipts()
            .iter()
            .map(|r| r.location())
            .collect::<Vec<_>>(),
        [
            IntentSourceLocation {
                event: 0,
                source: 0
            },
            IntentSourceLocation {
                event: 1,
                source: 0
            },
            IntentSourceLocation {
                event: 1,
                source: 1
            }
        ]
    );
    let SpeechUtteranceIntentEvent::Segment(original) = &source.events().as_slice()[0] else {
        panic!()
    };
    assert!(core::ptr::eq(
        prepared.receipts()[0].resolved().reference(),
        &original.sources().as_slice()[0]
    ));
    let ResolvedIntentSource::Phone(found) = prepared.receipts()[0].resolved() else {
        panic!()
    };
    assert!(core::ptr::eq(found.snapshot(), &phones));
    assert_eq!(found.token().phone(), &PhoneSpecification::unknown());
    let ResolvedIntentSource::Text(found) = prepared.receipts()[1].resolved() else {
        panic!()
    };
    assert_eq!(found.text(), "Qué");
    assert!(core::ptr::eq(found.material(), &text));
    let ResolvedIntentSource::Recognition {
        envelope: found,
        checked,
        ..
    } = prepared.receipts()[2].resolved()
    else {
        panic!()
    };
    assert!(core::ptr::eq(*found, &envelope));
    assert_eq!(checked.event_id(), envelope.event_id());
    assert_eq!(original.phone(), &PhoneSpecification::unknown());
}
#[test]
fn wrong_count_kind_revision_and_recognition_identity_refuse_exactly() {
    let source = intent(vec![vec![text_ref()], vec![recognition_ref()]]);
    let text = text("source revision");
    let good = envelope("stream", "event");
    assert!(matches!(
        resolve_intent_sources(&source, &[]),
        Err(IntentSourcesRefusal::MaterialCount {
            expected: 2,
            supplied: 0
        })
    ));
    let wrong = self::text("stale");
    assert!(matches!(
        resolve_intent_sources(
            &source,
            &[
                IntentSourceMaterial::Text(&wrong),
                IntentSourceMaterial::Recognition(&good)
            ]
        ),
        Err(IntentSourcesRefusal::Source {
            location: IntentSourceLocation {
                event: 0,
                source: 0
            },
            reason: IntentSourceReason::Text(_)
        })
    ));
    assert!(matches!(
        resolve_intent_sources(
            &source,
            &[
                IntentSourceMaterial::Recognition(&good),
                IntentSourceMaterial::Text(&text)
            ]
        ),
        Err(IntentSourcesRefusal::Source {
            reason: IntentSourceReason::MaterialKind,
            ..
        })
    ));
    for bad in [envelope("foreign", "event"), envelope("stream", "other")] {
        assert!(matches!(
            resolve_intent_sources(
                &source,
                &[
                    IntentSourceMaterial::Text(&text),
                    IntentSourceMaterial::Recognition(&bad)
                ]
            ),
            Err(IntentSourcesRefusal::Source {
                location: IntentSourceLocation {
                    event: 1,
                    source: 0
                },
                reason: IntentSourceReason::Recognition(_)
            })
        ));
    }
}
#[test]
fn empty_and_maximum_source_coverage_are_finite_and_complete() {
    let empty = intent(vec![]);
    assert!(resolve_intent_sources(&empty, &[])
        .unwrap()
        .receipts()
        .is_empty());
    // Collection bounds and the aggregate native node limit both apply.
    // Find this fixture's exact construction frontier; never enlarge a global
    // native profile to make a speech-only test pass.
    let (mut admitted, mut refused) = (1, 257);
    while refused - admitted > 1 {
        let count = (admitted + refused) / 2;
        match try_intent(vec![vec![text_ref(); 8]; count]) {
            Ok(_) => admitted = count,
            Err(conduit_plot::rust_binding::NativeBindingRefusal::InvalidValue(
                conduit_core::StructuredInfoRefusal::TooManyNodes
                | conduit_core::StructuredInfoRefusal::CanonicalEncodingTooLarge,
            )) => refused = count,
            Err(reason) => panic!("unexpected refusal: {reason:?}"),
        }
    }
    assert!(admitted < 256);
    let source = intent(vec![vec![text_ref(); 8]; admitted]);
    let text = text("source revision");
    let materials = vec![IntentSourceMaterial::Text(&text); admitted * 8];
    let prepared = resolve_intent_sources(&source, &materials).unwrap();
    assert_eq!(prepared.receipts().len(), admitted * 8);
    assert_eq!(
        prepared.receipts().last().unwrap().location(),
        IntentSourceLocation {
            event: admitted - 1,
            source: 7
        }
    );
    println!("eight-text-source fixture: {admitted} events / {} sources admitted; next event exceeds aggregate native profile", admitted * 8);
}

#[test]
fn phoneme_sources_retain_their_original_phone_evidence() {
    let (_, phones) = fixture::material(PhoneSpecification::unknown());
    let basis = phones.basis();
    let reference = LanguageSegmentRef::phoneme(
        basis.inventory_id().clone(),
        basis.language().clone(),
        0,
        basis.revision_id().clone(),
        basis.sequence_id().clone(),
        basis.utterance_id().clone(),
    )
    .unwrap();
    let token = SpeechPhonemeToken::new(
        phones.tokens().as_slice()[0].confidence().clone(),
        SpeechFeatureBundle::new(BoundedSequence::new()).unwrap(),
        PhonemeSpecification::unknown(),
        provenance(),
        BoundedSequence::try_from_iter([phones.tokens().as_slice()[0].clone()]).unwrap(),
        None,
    )
    .unwrap();
    let phonemes = SpeechPhonemeSequence::new(
        basis.clone(),
        BoundedSequence::try_from_iter([token]).unwrap(),
    )
    .unwrap();
    let source = intent(vec![vec![reference]]);
    let prepared =
        resolve_intent_sources(&source, &[IntentSourceMaterial::Phoneme(&phonemes)]).unwrap();
    let ResolvedIntentSource::Phoneme(found) = prepared.receipts()[0].resolved() else {
        panic!()
    };
    assert!(core::ptr::eq(found.snapshot(), &phonemes));
    assert!(core::ptr::eq(
        found.token(),
        &phonemes.tokens().as_slice()[0]
    ));
    assert_eq!(found.token().phoneme(), &PhonemeSpecification::unknown());
    assert_eq!(
        found.token().realized_as().as_slice()[0],
        phones.tokens().as_slice()[0]
    );
    assert!(matches!(
        resolve_intent_sources(&source, &[IntentSourceMaterial::Phone(&phones)]),
        Err(IntentSourcesRefusal::Source {
            reason: IntentSourceReason::MaterialKind,
            ..
        })
    ));
}
