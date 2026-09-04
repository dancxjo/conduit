use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembershipRevision, HostPresenceClock,
    HostPresenceClockScale, HostPresenceState, MembershipProofId, PartId,
};
use conduit_core::{BootId, CheckedFormId, HostId, OfferGeneration, SourceDocumentId};

fn empty_state() -> (AdmissionManager, BodyMembership, HostPresenceTable) {
    let body = Body::born(
        SourceDocumentId::from("source/atomic-return"),
        CheckedFormId::from("checked/atomic-return"),
        1,
        SignId::from("sign/atomic-return/body-born"),
    )
    .expect("body");
    (
        AdmissionManager::new(body.body_id.clone()).expect("admission"),
        BodyMembership::new(body.body_id.clone()).expect("membership"),
        HostPresenceTable::new(body.body_id, clock("empty"), 2_000).expect("presence"),
    )
}

fn credential() -> MembershipCredential {
    serde_json::from_str(
        r#"{"credential_id":"credential/atomic-return","body_id":"body/atomic-return","part_id":"part/atomic-return","host_id":"host/atomic-return","boot_id":"boot/atomic-return","issued_at_millis":1}"#,
    )
    .expect("credential")
}

fn current_state() -> (
    AdmissionManager,
    BodyMembership,
    HostPresenceTable,
    MembershipCredential,
) {
    let body = Body::born(
        SourceDocumentId::from("source/atomic-current"),
        CheckedFormId::from("checked/atomic-current"),
        1,
        SignId::from("sign/atomic-current/body-born"),
    )
    .expect("body");
    let body_id = body.body_id;
    let part_id = PartId::bind(&body_id, "part/browser", 1).expect("part");
    let mut membership = BodyMembership::new(body_id.clone()).expect("membership");
    membership
        .admit(
            &body_id,
            BodyMembershipRevision(0),
            part_id.clone(),
            MembershipProofId::bind("proof/atomic-admitted").expect("proof"),
            SignId::from("sign/atomic-current/admitted"),
        )
        .expect("admit");
    membership
        .observe_present(
            &body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/atomic-current"),
                boot_id: BootId::from("boot/atomic-current"),
                offer_generation: OfferGeneration(1),
                proof_id: MembershipProofId::bind("proof/atomic-attached").expect("proof"),
                sequence: 1,
            },
            SignId::from("sign/atomic-current/attached"),
        )
        .expect("attach");
    let mut presence =
        HostPresenceTable::new(body_id.clone(), clock("current"), 2_000).expect("presence");
    presence
        .start(
            &membership,
            &part_id,
            LinkBindingId::from("line/atomic-current"),
            1,
            1,
            100,
            SignId::from("sign/atomic-current/presence"),
        )
        .expect("start presence");
    let credential = serde_json::from_value(serde_json::json!({
        "credential_id": "credential/atomic-current",
        "body_id": body_id.as_str(),
        "part_id": part_id.as_str(),
        "host_id": "host/atomic-current",
        "boot_id": "boot/atomic-current",
        "issued_at_millis": 1
    }))
    .expect("credential");
    (
        AdmissionManager::new(body_id).expect("admission"),
        membership,
        presence,
        credential,
    )
}

fn clock(label: &str) -> HostPresenceClock {
    HostPresenceClock::new(
        format!("clock/return-admission/{label}"),
        HostPresenceClockScale::Milliseconds,
        1,
        0,
    )
    .unwrap()
}

fn assert_failed_transaction_is_unchanged(
    admission: &mut AdmissionManager,
    membership: &mut BodyMembership,
    presence: &mut HostPresenceTable,
    expected_error: &str,
) {
    let before_admission = admission.clone();
    let before_membership = membership.clone();
    let before_presence = presence.clone();
    let error = commit_return_atomically(
        admission,
        membership,
        presence,
        &credential(),
        LinkBindingId::from("line/atomic-return"),
        10,
        100,
        |next_admission, next_membership| {
            let other = Body::born(
                SourceDocumentId::from("source/mutated-clone"),
                CheckedFormId::from("checked/mutated-clone"),
                1,
                SignId::from("sign/mutated-clone/body-born"),
            )
            .expect("other body");
            next_admission.body_id = other.body_id.clone();
            next_membership.body_id = other.body_id;
            Ok(credential())
        },
    )
    .expect_err("transaction must refuse");
    assert!(error.contains(expected_error), "{error}");
    assert_eq!(*admission, before_admission);
    assert_eq!(*membership, before_membership);
    assert_eq!(*presence, before_presence);
}

#[test]
fn missing_retained_lease_refuses_without_committing_any_store() {
    let (mut admission, mut membership, mut presence) = empty_state();
    assert_failed_transaction_is_unchanged(
        &mut admission,
        &mut membership,
        &mut presence,
        "lease was not retained",
    );
}

#[test]
fn retained_sequence_overflow_refuses_without_committing_any_store() {
    let (mut admission, mut membership, mut presence) = empty_state();
    let credential = credential();
    presence.leases.push(conduit_body::HostPresenceLease {
        part_id: credential.part_id,
        host_id: credential.host_id,
        boot_id: credential.boot_id,
        offer_generation: conduit_core::OfferGeneration(1),
        membership_proof_id: conduit_body::MembershipProofId::bind("proof/atomic-return")
            .expect("proof id"),
        session_binding_id: "line/old-session".into(),
        sequence: u64::MAX,
        observed_at_millis: 1,
        expires_at_millis: 2,
        state: HostPresenceState::Unavailable,
    });
    assert_failed_transaction_is_unchanged(
        &mut admission,
        &mut membership,
        &mut presence,
        "sequence overflow",
    );
}

#[test]
fn malformed_presence_refuses_without_committing_any_store() {
    let (mut admission, mut membership, mut presence) = empty_state();
    let credential = credential();
    presence.leases.push(conduit_body::HostPresenceLease {
        part_id: credential.part_id,
        host_id: credential.host_id,
        boot_id: credential.boot_id,
        offer_generation: conduit_core::OfferGeneration(1),
        membership_proof_id: conduit_body::MembershipProofId::bind("proof/atomic-return")
            .expect("proof id"),
        session_binding_id: "line/old-session".into(),
        sequence: 1,
        observed_at_millis: 1,
        expires_at_millis: 2,
        state: HostPresenceState::Unavailable,
    });
    assert_failed_transaction_is_unchanged(
        &mut admission,
        &mut membership,
        &mut presence,
        "MalformedState",
    );
}

#[test]
fn available_presence_refuses_without_committing_any_store() {
    let (mut admission, mut membership, mut presence, credential) = current_state();
    let before_admission = admission.clone();
    let before_membership = membership.clone();
    let before_presence = presence.clone();
    let error = commit_return_atomically(
        &mut admission,
        &mut membership,
        &mut presence,
        &credential,
        LinkBindingId::from("line/atomic-return"),
        10,
        100,
        |next_admission, _| {
            let other = Body::born(
                SourceDocumentId::from("source/available-clone"),
                CheckedFormId::from("checked/available-clone"),
                1,
                SignId::from("sign/available-clone/body-born"),
            )
            .expect("other body");
            next_admission.body_id = other.body_id;
            Ok(credential.clone())
        },
    )
    .expect_err("available presence must refuse");
    assert!(error.contains("LeaseStillCurrent"), "{error}");
    assert_eq!(admission, before_admission);
    assert_eq!(membership, before_membership);
    assert_eq!(presence, before_presence);
}
