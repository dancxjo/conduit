use conduit_body::{
    AdmissionManager, BodyMembership, HostPresenceTable, MembershipCredential, PartReturnProof,
};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BROWSER_ADMISSION_PROTOCOL,
};
use std::time::Instant;

use super::return_session::wait_for_return_close;

#[allow(clippy::too_many_arguments)]
pub(super) fn accept_return(
    listener: &BrowserAdmissionListener,
    admission: &mut AdmissionManager,
    presence: &mut HostPresenceTable,
    membership: &mut BodyMembership,
    credential: &MembershipCredential,
    clock: Instant,
    lease_millis: u64,
    renew_after_millis: u64,
) -> Result<(), String> {
    let mut socket = listener
        .accept()
        .map_err(|error| format!("accept returning browser: {error:?}"))?;
    let (return_credential, advertisement) = match socket
        .receive()
        .map_err(|error| format!("receive return advertisement: {error:?}"))?
    {
        BrowserAdmissionIngress::ReturnAdvertise {
            credential: returned,
            advertisement,
            ..
        } if returned == *credential => (returned, advertisement),
        BrowserAdmissionIngress::ReturnAdvertise { .. } => {
            return Err("return used a stale membership credential".into());
        }
        _ => return Err("returning browser did not advertise continuity".into()),
    };
    let now_millis = monotonic_millis(clock)?;
    let return_challenge = admission
        .begin_return(
            membership,
            &return_credential.part_id,
            &advertisement,
            [7; 32],
            now_millis,
            now_millis + 1_000,
        )
        .map_err(|error| format!("begin browser return: {error:?}"))?;
    socket
        .send(&BrowserAdmissionEgress::ReturnChallenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge: return_challenge,
        })
        .map_err(|error| format!("send return challenge: {error:?}"))?;
    let proof = receive_return_proof(&mut socket)?;
    let return_session = LinkBindingId::from("line/browser-admission-probe/session-2");
    let observed_at_millis = monotonic_millis(clock)?;
    let (return_sequence, expires_at_millis) = commit_return_atomically(
        admission,
        membership,
        presence,
        credential,
        return_session.clone(),
        observed_at_millis,
        lease_millis,
        |next_admission, next_membership| {
            next_admission
                .complete_return(
                    next_membership,
                    &advertisement,
                    &proof,
                    observed_at_millis,
                    SignId::from("sign/browser-admission-probe/returned"),
                )
                .map_err(|error| format!("complete browser return: {error:?}"))
        },
    )?;
    if let Err(error) = socket.send(&BrowserAdmissionEgress::PresenceAccepted {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        sequence: return_sequence,
        renew_after_millis,
        expires_at_millis,
    }) {
        presence
            .lose_session(
                membership,
                &credential.part_id,
                &return_session,
                monotonic_millis(clock)?,
                SignId::from("sign/browser-admission-probe/return-acceptance-lost"),
            )
            .map_err(|refusal| format!("record failed return acceptance: {refusal:?}"))?;
        return Err(format!("send returned presence acceptance: {error:?}"));
    }
    println!(
        "returned part={} host={} boot={} sequence={return_sequence}",
        credential.part_id.as_str(),
        credential.host_id.as_str(),
        credential.boot_id.as_str()
    );
    wait_for_return_close(
        &mut socket,
        presence,
        membership,
        credential,
        &return_session,
        clock,
        lease_millis,
        renew_after_millis,
    )
}

fn receive_return_proof(
    socket: &mut conduit_std_host::browser_admission::BrowserAdmissionSocket,
) -> Result<PartReturnProof, String> {
    match socket
        .receive()
        .map_err(|error| format!("receive return proof: {error:?}"))?
    {
        BrowserAdmissionIngress::ReturnProof {
            admission_id,
            body_id,
            part_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => Ok(PartReturnProof {
            admission_id,
            body_id,
            part_id,
            host_id,
            boot_id,
            nonce: nonce.try_into().map_err(|_| "invalid return nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid return signature")?,
        }),
        _ => Err("returning browser did not prove continuity".into()),
    }
}

#[allow(clippy::too_many_arguments)]
fn commit_return_atomically<F>(
    admission: &mut AdmissionManager,
    membership: &mut BodyMembership,
    presence: &mut HostPresenceTable,
    credential: &MembershipCredential,
    session: LinkBindingId,
    observed_at_millis: u64,
    lease_millis: u64,
    complete_membership: F,
) -> Result<(u64, u64), String>
where
    F: FnOnce(&mut AdmissionManager, &mut BodyMembership) -> Result<(), String>,
{
    let mut next_admission = admission.clone();
    let mut next_membership = membership.clone();
    let mut next_presence = presence.clone();
    complete_membership(&mut next_admission, &mut next_membership)?;
    let sequence = next_presence
        .leases
        .iter()
        .find(|lease| lease.part_id == credential.part_id)
        .ok_or("return presence lease was not retained")?
        .sequence
        .checked_add(1)
        .ok_or("return presence sequence overflow")?;
    next_presence
        .start(
            &next_membership,
            &credential.part_id,
            session,
            sequence,
            observed_at_millis,
            lease_millis,
            SignId::from("sign/browser-admission-probe/return-presence"),
        )
        .map_err(|error| format!("start returned presence: {error:?}"))?;
    let expires_at_millis = next_presence
        .leases
        .iter()
        .find(|lease| lease.part_id == credential.part_id)
        .ok_or("returned presence lease disappeared")?
        .expires_at_millis;
    *admission = next_admission;
    *membership = next_membership;
    *presence = next_presence;
    Ok((sequence, expires_at_millis))
}

fn monotonic_millis(clock: Instant) -> Result<u64, String> {
    u64::try_from(clock.elapsed().as_millis())
        .map_err(|_| "return presence clock overflowed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        AuthenticatedHostObservation, Body, BodyMembershipRevision, HostPresenceState,
        MembershipProofId, PartId,
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
            HostPresenceTable::new(body.body_id, 2_000).expect("presence"),
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
        let mut presence = HostPresenceTable::new(body_id.clone(), 2_000).expect("presence");
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
                Ok(())
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
                Ok(())
            },
        )
        .expect_err("available presence must refuse");
        assert!(error.contains("LeaseStillCurrent"), "{error}");
        assert_eq!(admission, before_admission);
        assert_eq!(membership, before_membership);
        assert_eq!(presence, before_presence);
    }
}
