extern crate alloc;

use alloc::vec;
use conduit_core::SignId;
use conduit_presentation::*;

fn instant(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Seconds,
        clock_basis: "clock/model-turn".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn presentation(reference_ticks: u64) -> Presentation {
    let reference = TemporalReference {
        identity: "reference/model-turn".into(),
        instant: instant(reference_ticks),
    };
    let fact = PresentationTemporalFact::new(
        "subject/battery".into(),
        PresentationTemporalRole::Observation,
        Some(SignId::from("sign/battery-observation")),
        instant(100),
        &reference,
    )
    .unwrap();
    Presentation::new_with_semantics_and_temporal(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![SignId::from("sign/battery-observation")],
        },
        vec![PresentationSubject {
            identity: "subject/battery".into(),
            role: PresentationRole::Diagnostic,
            label: "Battery".into(),
            accessibility_name: "Battery observation".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![reference],
        vec![fact],
    )
    .unwrap()
}

#[test]
fn model_context_leads_with_age_and_keeps_exact_provenance() {
    let projected = project_model_temporal_context(&presentation(118)).unwrap();
    let fact = &projected[0];
    assert_eq!(fact.relative_time, "18 seconds ago");
    assert_eq!(
        fact.relation,
        TemporalRelation::Past {
            minimum_ticks: 18,
            maximum_ticks: 18,
        }
    );
    assert_eq!(fact.source, instant(100));
    assert_eq!(fact.reference.instant, instant(118));
    assert_eq!(fact.role, PresentationTemporalRole::Observation);
    assert_eq!(fact.sign_id, Some(SignId::from("sign/battery-observation")));

    let json = serde_json::to_string(fact).unwrap();
    assert!(json.starts_with("{\"relative_time\":\"18 seconds ago\",\"relation\":"));
    assert!(json.contains("\"source\":{\"ticks\":100"));
    assert!(json.contains("\"reference\":{\"identity\":\"reference/model-turn\""));
    assert!(json.contains("\"clock_basis\":\"clock/model-turn\""));
}

#[test]
fn new_reference_recomputes_context_without_mutating_observation_evidence() {
    let earlier = project_model_temporal_context(&presentation(90)).unwrap();
    let later = project_model_temporal_context(&presentation(130)).unwrap();
    assert_eq!(earlier[0].relative_time, "in 10 seconds");
    assert_eq!(later[0].relative_time, "30 seconds ago");
    assert_eq!(earlier[0].source, later[0].source);
    assert_eq!(earlier[0].subject, later[0].subject);
    assert_eq!(earlier[0].role, later[0].role);
    assert_eq!(earlier[0].sign_id, later[0].sign_id);
    assert_ne!(earlier[0].reference, later[0].reference);
}

#[test]
fn malformed_presentation_refuses_before_model_projection() {
    let mut value = presentation(118);
    value.temporal_facts[0].reference = "reference/missing".into();
    assert_eq!(
        project_model_temporal_context(&value),
        Err(PresentationError::UnknownTemporalReference)
    );
}
