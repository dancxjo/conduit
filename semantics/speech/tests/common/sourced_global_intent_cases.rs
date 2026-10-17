use super::*;

#[test]
fn sourced_global_admission_retains_exact_coverage_and_refuses_stale_late_material() {
    use conduit_speech::{
        intent_sources::{IntentSourceMaterial, IntentSourcesRefusal, ResolvedIntentSource},
        sourced_global_intent::*,
    };
    let source = occurrence::intent([segment(10), boundary(), segment(11)]);
    let setup = Setup::new();
    let defaults = [default(10), default(11)];
    let evidence = [
        setup.evidence(&defaults[0]),
        None,
        setup.evidence(&defaults[1]),
    ];
    let text = |revision: &str| {
        LanguageText::new(
            LanguageTextId::new("source".into()).unwrap(),
            LanguageId::new("en".into()).unwrap(),
            LanguageTextRevisionId::new(revision.into()).unwrap(),
            "t".into(),
        )
        .unwrap()
    };
    let material = text("source revision");
    let stale = text("stale revision");
    let materials = [IntentSourceMaterial::Text(&material); 3];
    let prepare = |materials| {
        prepare_sourced_global_intent(
            &source,
            materials,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &evidence,
        )
    };
    let prepared = prepare(&materials).unwrap();
    assert!(core::ptr::eq(prepared.source(), &source));
    assert!(core::ptr::eq(
        prepared.sources().intent(),
        prepared.realization().source()
    ));
    for (event, receipt) in prepared.sources().receipts().iter().enumerate() {
        assert_eq!(receipt.location().event, event);
        assert_eq!(receipt.location().source, 0);
        let ResolvedIntentSource::Text(resolved) = receipt.resolved() else {
            panic!()
        };
        assert!(core::ptr::eq(resolved.material(), &material));
        assert_eq!(resolved.text(), "t");
        let reference = match &source.events().as_slice()[event] {
            SpeechUtteranceIntentEvent::Segment(value) => &value.sources().as_slice()[0],
            SpeechUtteranceIntentEvent::Boundary(value) => &value.sources().as_slice()[0],
        };
        assert!(core::ptr::eq(resolved.reference(), reference));
    }
    assert_eq!(
        pcm(prepared.renderer().unwrap(), 1),
        pcm(prepared.realization().renderer().unwrap(), 128)
    );
    assert!(matches!(
        prepare(&materials[..2]),
        Err(SourcedGlobalRefusal::Sources(
            IntentSourcesRefusal::MaterialCount {
                expected: 3,
                supplied: 2
            }
        ))
    ));
    let late = [
        materials[0],
        materials[1],
        IntentSourceMaterial::Text(&stale),
    ];
    let Err(SourcedGlobalRefusal::Sources(IntentSourcesRefusal::Source { location, .. })) =
        prepare(&late)
    else {
        panic!()
    };
    assert_eq!(location.event, 2);
    assert_eq!(location.source, 0);
    assert!(matches!(
        prepare_sourced_global_intent(
            &source,
            &late,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[],
        ),
        Err(SourcedGlobalRefusal::Sources(_))
    ));
    assert!(matches!(
        prepare_sourced_global_intent(
            &source,
            &materials,
            &setup.inventory,
            &setup.voice,
            &setup.boundaries,
            &setup.rules,
            &setup.policy,
            &[],
        ),
        Err(SourcedGlobalRefusal::Realization(
            GlobalIntentRefusal::EvidenceCount
        ))
    ));
}
