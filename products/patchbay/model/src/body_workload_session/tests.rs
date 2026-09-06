use super::*;
use conduit_body::{
    Body, BodyBiographyEvidence, BodyGraduationChoice, BodyGraduationEvidence, BodyMembership,
};
use conduit_core::{CheckedFormId, SourceDocumentId};

fn form(name: &str) -> ResidentForm {
    ResidentForm::new(
        SourceDocumentId::from(format!("source/{name}")),
        CheckedFormId::from(format!("checked/{name}")),
    )
}

fn session() -> PatchbayBodyWorkloadSession {
    let body = Body::born_with_forms(
        conduit_body::BodyWorkset::from_forms([form("clock"), form("lantern")]).unwrap(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence = BodyBiographyEvidence::born(body, membership, "Talvi".into()).unwrap();
    evidence
        .graduate(BodyGraduationEvidence {
            body_id: evidence.body_id.clone(),
            sequence: 2,
            sign_id: SignId::from("sign/graduated"),
            choice: BodyGraduationChoice::ExternalReader,
            patchbay_plan_id: None,
            patchbay_implementation_id: None,
        })
        .unwrap();
    PatchbayBodyWorkloadSession::open_serialized(
        &serde_json::to_vec(&evidence).unwrap(),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap()
}

#[test]
fn add_then_remove_preserves_body_identity_and_advances_exact_workload_evidence() {
    let mut session = session();
    let body_id = session.evidence().body_id.clone();
    let telegraph = form("telegraph");
    let admitted = session
        .admit_form(
            0,
            telegraph.clone(),
            SignId::from("sign/telegraph-admitted"),
            3,
        )
        .unwrap();
    assert_eq!(admitted.prior_workload_revision, 0);
    assert_eq!(admitted.workload_revision, 1);
    assert_eq!(admitted.kind, BodyWorkloadChangeKind::Admitted);
    assert_eq!(session.evidence().body_id, body_id);
    assert!(session.evidence().body.workset.contains(&telegraph));

    let removed = session
        .remove_form(1, form("clock"), SignId::from("sign/clock-removed"), 4)
        .unwrap();
    assert_eq!(removed.workload_revision, 2);
    assert_eq!(removed.kind, BodyWorkloadChangeKind::Removed);
    assert_eq!(session.evidence().body_id, body_id);
    assert_eq!(session.evidence().body.workset.len(), 2);

    let reopened = PatchbayBodyWorkloadSession::open_serialized(
        session.encoded_evidence(),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    assert_eq!(reopened.evidence(), session.evidence());
}

#[test]
fn stale_duplicate_absent_and_reused_evidence_fail_without_partial_mutation() {
    let mut session = session();
    let original = session.encoded_evidence().to_vec();
    assert_eq!(
        session.admit_form(9, form("radio"), SignId::from("sign/radio"), 3),
        Err(PatchbayBodyWorkloadError::StaleWorkloadRevision {
            current: 0,
            offered: 9,
        })
    );
    assert!(matches!(
        session.admit_form(0, form("clock"), SignId::from("sign/duplicate"), 3),
        Err(PatchbayBodyWorkloadError::Lifecycle(
            BodyLifecycleError::DuplicateForm
        ))
    ));
    assert!(matches!(
        session.remove_form(0, form("absent"), SignId::from("sign/absent"), 3),
        Err(PatchbayBodyWorkloadError::Lifecycle(
            BodyLifecycleError::FormAbsent
        ))
    ));
    assert!(matches!(
        session.admit_form(0, form("radio"), SignId::from("sign/graduated"), 3),
        Err(PatchbayBodyWorkloadError::Biography(
            BodyBiographyError::InvalidSequence
        ))
    ));
    assert_eq!(session.encoded_evidence(), original);
}

#[test]
fn awake_body_refuses_workload_change_instead_of_leaving_a_wake_stale() {
    let lulled = session();
    let mut evidence = lulled.evidence().clone();
    let (awake, wake) = evidence.body.wake(3, SignId::from("sign/woke")).unwrap();
    evidence.append_wake(awake, wake, 3).unwrap();
    let mut session = PatchbayBodyWorkloadSession::open_serialized(
        &serde_json::to_vec(&evidence).unwrap(),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    let before = session.encoded_evidence().to_vec();

    assert_eq!(
        session.admit_form(0, form("radio"), SignId::from("sign/radio"), 4),
        Err(PatchbayBodyWorkloadError::BodyAwake)
    );
    assert_eq!(session.encoded_evidence(), before);
}

#[test]
fn retained_lull_allows_later_workload_changes_and_readable_history() {
    let mut session = session();
    let id = session.evidence().body_id.clone();
    let (body, wake) = session
        .evidence()
        .body
        .wake(1, SignId::from("sign/woke"))
        .unwrap();
    session.retain_wake(body.clone(), wake.clone(), 3).unwrap();
    let before = session.encoded_evidence().to_vec();
    assert!(session.retain_wake(body.clone(), wake.clone(), 3).is_err());
    assert_eq!(session.encoded_evidence(), before);
    let wake = wake.lull(SignId::from("sign/lull")).unwrap();
    let body = body
        .retain_after_lull(&wake, SignId::from("sign/retained"))
        .unwrap();
    session.retain_wake(body, wake.clone(), 4).unwrap();
    session
        .admit_form(0, form("radio"), SignId::from("sign/radio"), 6)
        .unwrap();
    assert_eq!(session.evidence().body_id, id);
    assert_eq!(session.evidence().wakes, vec![wake]);
    let reopened = PatchbayBodyWorkloadSession::open_serialized(
        session.encoded_evidence(),
        PatchbayBodyApplicationEntrance::ExternalReader,
    )
    .unwrap();
    let projection = crate::project_body_biography(reopened.evidence()).unwrap();
    let headings: Vec<_> = projection
        .entries
        .iter()
        .map(|entry| entry.heading)
        .collect();
    assert_eq!(
        &headings[2..],
        &[
            "Woke",
            "Wake lulled",
            "Body retained after Lull",
            "Form admitted"
        ]
    );
}
