use conduit_body::{
    ContinuousDisposition, ContinuousError, ContinuousResourceAdmission, ContinuousSpecimen,
};
use conduit_core::{CheckedFormId, PlanId, SourceDocumentId};

fn specimen() -> ContinuousSpecimen {
    ContinuousSpecimen::admit(
        SourceDocumentId::from("source/thermostat"),
        CheckedFormId::from("checked/thermostat"),
        PlanId::from("plan/a"),
        ContinuousResourceAdmission::specimen(),
    )
    .unwrap()
}

#[test]
fn finite_state_form_accepts_continuation_without_a_fixed_transition_count() {
    let mut specimen = specimen();
    for _ in 0..1_024 {
        assert_eq!(specimen.accept(1), Ok(ContinuousDisposition::Continued));
    }
    assert_eq!(specimen.state, 1_024);
    assert_eq!(specimen.resources, ContinuousResourceAdmission::specimen());
}

#[test]
fn replan_preserves_form_and_retained_state_but_changes_realization() {
    let mut specimen = specimen();
    specimen.accept(7).unwrap();
    let source = specimen.source_document_id.clone();
    let checked = specimen.checked_form_id.clone();
    let state = specimen.state;

    assert_eq!(
        specimen.replan(PlanId::from("plan/b")),
        Ok(ContinuousDisposition::Replanned)
    );
    assert_eq!(specimen.source_document_id, source);
    assert_eq!(specimen.checked_form_id, checked);
    assert_eq!(specimen.state, state);
    assert_eq!(specimen.plan_id, PlanId::from("plan/b"));
    assert_eq!(specimen.accept(1), Ok(ContinuousDisposition::Continued));
}

#[test]
fn terminal_and_nonterminal_dispositions_remain_distinct() {
    let mut specimen = specimen();
    assert_ne!(
        specimen.accept(1).unwrap(),
        ContinuousDisposition::SemanticCompletion
    );
    assert_eq!(
        specimen.complete(),
        ContinuousDisposition::SemanticCompletion
    );
    assert_eq!(specimen.accept(1), Err(ContinuousError::NotLive));
    assert_eq!(
        specimen.disposition,
        ContinuousDisposition::SemanticCompletion
    );
}

#[test]
fn admission_rejects_zero_capacity() {
    let mut resources = ContinuousResourceAdmission::specimen();
    resources.queue_slots = 0;
    assert_eq!(
        ContinuousSpecimen::admit(
            SourceDocumentId::from("source"),
            CheckedFormId::from("checked"),
            PlanId::from("plan"),
            resources,
        ),
        Err(ContinuousError::InvalidAdmission)
    );
}
