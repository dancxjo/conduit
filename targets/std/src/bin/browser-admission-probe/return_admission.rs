use conduit_body::{
    AdmissionManager, BodyBiographyEvidence, BodyMembership, CandidateObservation,
    DiscoveryProofId, HostPresenceTable, MembershipCredential, PartReturnProof,
};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BROWSER_ADMISSION_PROTOCOL,
};
use std::time::Instant;

use super::offer_evidence::send_admitted_offer_evidence;
use super::return_session::wait_for_return_close;

#[allow(clippy::too_many_arguments)]
pub(super) fn accept_return(
    listener: &BrowserAdmissionListener,
    admission: &mut AdmissionManager,
    presence: &mut HostPresenceTable,
    membership: &mut BodyMembership,
    biography: &mut BodyBiographyEvidence,
    credential: &MembershipCredential,
    clock: Instant,
    lease_millis: u64,
    renew_after_millis: u64,
) -> Result<(), String> {
    let mut socket = listener
        .accept()
        .map_err(|error| format!("accept returning browser: {error:?}"))?;
    let (frame, encoded_bytes) = socket
        .receive_with_size()
        .map_err(|error| format!("receive return advertisement: {error:?}"))?;
    let (return_credential, advertisement) = match frame {
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
    let return_sign = SignId::from("sign/browser-admission-probe/returned");
    let offer_observation = CandidateObservation {
        advertisement: advertisement.clone(),
        friendly_label: "Returning browser".into(),
        observed_binding_id: LinkBindingId::from(
            "line/browser-admission-probe/return-advertisement",
        ),
        observation_sign_id: return_sign.clone(),
        proof_id: DiscoveryProofId::bind(proof.admission_id.as_str())
            .map_err(|error| format!("bind return offer proof: {error:?}"))?,
        freshness_sequence: membership
            .revision
            .0
            .checked_add(1)
            .ok_or("return offer freshness sequence exhausted")?,
        encoded_bytes,
    };
    let return_session = LinkBindingId::from("line/browser-admission-probe/session-2");
    let observed_at_millis = monotonic_millis(clock)?;
    let prior_membership_events = membership.events.len();
    let biography_is_current = biography.membership == *membership;
    let (returned_credential, return_sequence, expires_at_millis) = commit_return_atomically(
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
                    return_sign,
                )
                .map_err(|error| format!("complete browser return: {error:?}"))
        },
    )?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: returned_credential.clone(),
        })
        .map_err(|error| format!("send renewed return credential: {error:?}"))?;
    if biography_is_current {
        let returned_event = membership
            .events
            .get(prior_membership_events)
            .ok_or("browser return did not append membership evidence")?;
        let biography_sequence = biography
            .records
            .last()
            .and_then(|record| record.sequence.checked_add(1))
            .ok_or("Body biography sequence exhausted")?;
        biography
            .append_membership_events(
                membership.clone(),
                &[(returned_event.change_id.clone(), biography_sequence)],
            )
            .map_err(|error| format!("append return biography: {error:?}"))?;
        socket
            .send(&BrowserAdmissionEgress::BiographyEvidence {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                evidence: Box::new(biography.clone()),
            })
            .map_err(|error| format!("send return biography evidence: {error:?}"))?;
    }
    send_admitted_offer_evidence(&mut socket, &offer_observation)?;
    if let Err(error) = socket.send(&BrowserAdmissionEgress::PresenceAccepted {
        protocol: BROWSER_ADMISSION_PROTOCOL,
        sequence: return_sequence,
        renew_after_millis,
        expires_at_millis,
    }) {
        presence
            .lose_session(
                membership,
                &returned_credential.part_id,
                &return_session,
                monotonic_millis(clock)?,
                SignId::from("sign/browser-admission-probe/return-acceptance-lost"),
            )
            .map_err(|refusal| format!("record failed return acceptance: {refusal:?}"))?;
        return Err(format!("send returned presence acceptance: {error:?}"));
    }
    println!(
        "returned part={} host={} boot={} sequence={return_sequence}",
        returned_credential.part_id.as_str(),
        returned_credential.host_id.as_str(),
        returned_credential.boot_id.as_str()
    );
    wait_for_return_close(
        &mut socket,
        presence,
        membership,
        &returned_credential,
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
) -> Result<(MembershipCredential, u64, u64), String>
where
    F: FnOnce(&mut AdmissionManager, &mut BodyMembership) -> Result<MembershipCredential, String>,
{
    let mut next_admission = admission.clone();
    let mut next_membership = membership.clone();
    let mut next_presence = presence.clone();
    let returned_credential = complete_membership(&mut next_admission, &mut next_membership)?;
    if returned_credential.body_id != credential.body_id
        || returned_credential.part_id != credential.part_id
        || returned_credential.host_id != credential.host_id
    {
        return Err("renewed return credential changed durable membership identity".into());
    }
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
    Ok((returned_credential, sequence, expires_at_millis))
}

fn monotonic_millis(clock: Instant) -> Result<u64, String> {
    u64::try_from(clock.elapsed().as_millis())
        .map_err(|_| "return presence clock overflowed".into())
}

#[cfg(test)]
#[path = "return_admission/tests.rs"]
mod tests;
