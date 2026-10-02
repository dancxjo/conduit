//! Explicit owner authorization followed by canonical proof verification.
use super::{nonce, now, signal, transport::Socket, Owner, PROTOCOL};
use conduit_body::{
    AdmissionManager, AmbientAdmissionProof, CandidateInventory, CandidateObservation,
    DiscoveryProofId, MembershipCredential, PartReturnProof,
};
use conduit_core::{HostId, LinkBindingId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
};
use std::time::{Duration, Instant};

pub(super) fn admit(
    owner: &mut Owner,
    socket: &mut Socket,
    expected: &HostId,
    new_host_key: Option<[u8; 32]>,
    binding: &LinkBindingId,
    clock: Instant,
    deadline: Instant,
) -> Result<(MembershipCredential, CandidateObservation), String> {
    let (frame, encoded_bytes) =
        socket.receive(remaining(deadline)?.min(Duration::from_secs(2)))?;
    let mut manager = owner.admissions.clone().unwrap_or(
        AdmissionManager::new(owner.session.evidence().body_id.clone()).map_err(super::debug)?,
    );
    let authority = owner.host.advertisement();
    let host = authority.host_id.clone();
    let boot = authority.boot_id.clone();
    match frame {
        In::Advertise {
            protocol: PROTOCOL,
            advertisement,
            friendly_label,
            verifying_key,
            freshness_sequence,
        } => {
            if &advertisement.host_id != expected {
                return Err("browser Host differs from explicit admission authorization".into());
            }
            if owner
                .session
                .evidence()
                .membership
                .parts
                .iter()
                .any(|p| p.current.as_ref().is_some_and(|h| &h.host_id == expected))
            {
                return Err("browser Host already has current membership".into());
            }
            if manager
                .receipts
                .iter()
                .any(|r| &r.credential.host_id == expected)
            {
                return Err("admitted browser must present its retained credential".into());
            }
            let offered_key = authorize_new_key(new_host_key, verifying_key)?;
            let observation = CandidateObservation {
                advertisement,
                friendly_label,
                freshness_sequence,
                encoded_bytes,
                observed_binding_id: binding.clone(),
                observation_sign_id: signal(binding, "observed"),
                proof_id: DiscoveryProofId::bind(binding.as_str()).map_err(super::debug)?,
            };
            let mut candidates =
                CandidateInventory::new(manager.body_id.clone()).map_err(super::debug)?;
            let candidate = candidates
                .observe(observation.clone())
                .map_err(super::debug)?;
            let at = now(clock);
            let expires = expiration(clock, deadline, at)?;
            let challenge = manager
                .begin_ambient(
                    &mut candidates,
                    &candidate,
                    offered_key,
                    nonce()?,
                    at,
                    expires,
                    signal(binding, "requested"),
                )
                .map_err(super::debug)?;
            socket.send(&Out::Challenge {
                protocol: PROTOCOL,
                challenge,
            })?;
            let (
                In::AmbientProof {
                    protocol: PROTOCOL,
                    admission_id,
                    body_id,
                    host_id,
                    boot_id,
                    nonce,
                    signature,
                },
                _,
            ) = socket.receive(remaining(deadline)?.min(Duration::from_secs(2)))?
            else {
                return Err("expected browser ambient proof".into());
            };
            let proof = AmbientAdmissionProof {
                admission_id,
                body_id,
                host_id,
                boot_id,
                nonce: nonce.try_into().map_err(|_| "invalid proof nonce")?,
                signature: signature
                    .try_into()
                    .map_err(|_| "invalid proof signature")?,
            };
            remaining(deadline)?;
            let mut session = owner.session.clone();
            let credential = session
                .admit_ambient_host(
                    &mut manager,
                    &mut candidates,
                    &proof,
                    now(clock),
                    &host,
                    &boot,
                )
                .map_err(super::debug)?;
            remaining(deadline)?;
            owner.session = session;
            owner.admissions = Some(manager);
            Ok((credential, observation))
        }
        In::ReturnAdvertise {
            protocol: PROTOCOL,
            credential,
            advertisement,
        } => {
            if &advertisement.host_id != expected || &credential.host_id != expected {
                return Err("returning browser Host differs from explicit authorization".into());
            }
            if manager
                .receipts
                .iter()
                .rev()
                .find(|r| r.credential.part_id == credential.part_id)
                .is_none_or(|r| r.credential != credential)
            {
                return Err("returning browser credential is stale or unknown".into());
            }
            let at = now(clock);
            let expires = expiration(clock, deadline, at)?;
            let challenge = manager
                .begin_return(
                    &owner.session.evidence().membership,
                    &credential.part_id,
                    &advertisement,
                    nonce()?,
                    at,
                    expires,
                )
                .map_err(super::debug)?;
            socket.send(&Out::ReturnChallenge {
                protocol: PROTOCOL,
                challenge,
            })?;
            let (
                In::ReturnProof {
                    protocol: PROTOCOL,
                    admission_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    nonce,
                    signature,
                },
                _,
            ) = socket.receive(remaining(deadline)?.min(Duration::from_secs(2)))?
            else {
                return Err("expected browser return proof".into());
            };
            let proof = PartReturnProof {
                admission_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                nonce: nonce.try_into().map_err(|_| "invalid return nonce")?,
                signature: signature
                    .try_into()
                    .map_err(|_| "invalid return signature")?,
            };
            remaining(deadline)?;
            let mut session = owner.session.clone();
            let credential = session
                .admit_returning_host(
                    &mut manager,
                    &advertisement,
                    &proof,
                    now(clock),
                    &host,
                    &boot,
                )
                .map_err(super::debug)?;
            let observation = CandidateObservation {
                advertisement,
                friendly_label: "Returning browser".into(),
                observed_binding_id: binding.clone(),
                observation_sign_id: signal(binding, "returned"),
                proof_id: DiscoveryProofId::bind(proof.admission_id.as_str())
                    .map_err(super::debug)?,
                freshness_sequence: session.evidence().membership.revision.0,
                encoded_bytes,
            };
            remaining(deadline)?;
            owner.session = session;
            owner.admissions = Some(manager);
            Ok((credential, observation))
        }
        _ => Err("first browser frame must advertise admission or return".into()),
    }
}

fn authorize_new_key(expected: Option<[u8; 32]>, offered: Vec<u8>) -> Result<[u8; 32], String> {
    let offered: [u8; 32] = offered
        .try_into()
        .map_err(|_| "invalid browser verifying key")?;
    if expected != Some(offered) {
        return Err("new browser proof key differs from explicit admission authorization".into());
    }
    Ok(offered)
}

/// The carrier has bounded per-read timeouts, not an absolute I/O deadline.
/// Expiry is therefore checked again before publishing any admission decision.
pub(super) fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| "browser admission authorization window expired".into())
}
fn expiration(clock: Instant, deadline: Instant, at: u64) -> Result<u64, String> {
    remaining(deadline)?;
    let expires = (deadline.duration_since(clock).as_millis() as u64).min(at + 2000);
    if expires <= at {
        return Err("browser admission authorization window expired".into());
    }
    Ok(expires)
}

#[cfg(test)]
mod tests {
    use super::{authorize_new_key, expiration, remaining};
    use std::time::{Duration, Instant};

    #[test]
    fn challenge_expiry_is_capped_by_authorization_and_elapsed_windows_refuse() {
        let clock = Instant::now();
        assert_eq!(
            expiration(clock, clock + Duration::from_secs(1), 0).unwrap(),
            1000
        );
        assert_eq!(
            expiration(clock, clock + Duration::from_secs(60), 100).unwrap(),
            2100
        );
        assert!(remaining(clock - Duration::from_millis(1)).is_err());
        assert!(expiration(clock, clock - Duration::from_millis(1), 0).is_err());
    }
    #[test]
    fn claiming_the_authorized_host_does_not_authorize_a_different_or_unpinned_key() {
        assert!(authorize_new_key(None, vec![7; 32]).is_err());
        assert!(authorize_new_key(Some([7; 32]), vec![8; 32]).is_err());
        assert!(authorize_new_key(Some([7; 32]), vec![7; 31]).is_err());
        assert_eq!(
            authorize_new_key(Some([7; 32]), vec![7; 32]).unwrap(),
            [7; 32]
        );
    }
}
