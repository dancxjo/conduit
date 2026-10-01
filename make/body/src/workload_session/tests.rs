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

fn session() -> BodyWorkloadSession {
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
            reader_plan_id: None,
            reader_implementation_id: None,
        })
        .unwrap();
    BodyWorkloadSession::open_serialized(&serde_json::to_vec(&evidence).unwrap()).unwrap()
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

    let reopened = BodyWorkloadSession::open_serialized(session.encoded_evidence()).unwrap();
    assert_eq!(reopened.evidence(), session.evidence());
}

#[test]
fn stale_duplicate_absent_and_reused_evidence_fail_without_partial_mutation() {
    let mut session = session();
    let original = session.encoded_evidence().to_vec();
    assert_eq!(
        session.admit_form(9, form("radio"), SignId::from("sign/radio"), 3),
        Err(BodyWorkloadError::StaleWorkloadRevision {
            current: 0,
            offered: 9,
        })
    );
    assert!(matches!(
        session.admit_form(0, form("clock"), SignId::from("sign/duplicate"), 3),
        Err(BodyWorkloadError::Lifecycle(
            BodyLifecycleError::DuplicateForm
        ))
    ));
    assert!(matches!(
        session.remove_form(0, form("absent"), SignId::from("sign/absent"), 3),
        Err(BodyWorkloadError::Lifecycle(BodyLifecycleError::FormAbsent))
    ));
    assert!(matches!(
        session.admit_form(0, form("radio"), SignId::from("sign/graduated"), 3),
        Err(BodyWorkloadError::Biography(
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
    let mut session =
        BodyWorkloadSession::open_serialized(&serde_json::to_vec(&evidence).unwrap()).unwrap();
    let before = session.encoded_evidence().to_vec();

    assert_eq!(
        session.admit_form(0, form("radio"), SignId::from("sign/radio"), 4),
        Err(BodyWorkloadError::BodyAwake)
    );
    assert_eq!(session.encoded_evidence(), before);
}

#[test]
fn retained_lull_allows_later_workload_changes() {
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
    let reopened = BodyWorkloadSession::open_serialized(session.encoded_evidence()).unwrap();
    assert_eq!(reopened.evidence(), session.evidence());
}

#[test]
fn malformed_invalid_and_oversized_documents_fail_closed() {
    assert!(matches!(
        BodyWorkloadSession::open_serialized(&[]),
        Err(BodyWorkloadError::EmptyEvidence)
    ));
    assert!(matches!(
        BodyWorkloadSession::open_serialized(b"{not-json"),
        Err(BodyWorkloadError::MalformedEvidence)
    ));
    let mut invalid = session().evidence().clone();
    invalid.schema = "conduit.body/biography-evidence@future".into();
    assert!(matches!(
        BodyWorkloadSession::open_serialized(&serde_json::to_vec(&invalid).unwrap()),
        Err(BodyWorkloadError::InvalidEvidence)
    ));
    assert!(matches!(
        BodyWorkloadSession::open_serialized(&vec![b' '; MAX_BODY_EVIDENCE_BYTES + 1]),
        Err(BodyWorkloadError::EvidenceTooLarge)
    ));
}
