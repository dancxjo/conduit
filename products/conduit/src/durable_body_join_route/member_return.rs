//! Finite pinned entrance for an existing member, never a spawn invitation.
use super::*;
use crate::durable_host::owner::{MemberPresentationBasis, MemberReturnProofDocument};
use conduit_body::{MembershipCredential, PartReturnChallenge};
use conduit_core::HostAdvertisement;

pub(crate) const MEMBER_ROUTE_SCHEMA: &str = "conduit.body/member-encounter@1";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberEncounter {
    pub schema: String,
    pub body_id: conduit_body::BodyId,
    pub part_id: conduit_body::PartId,
    pub host_id: conduit_core::HostId,
    pub candidate: RendezvousCandidate,
    pub owner: conduit_body::FederationPeer,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum MemberReturnFrame {
    DescribeContact {
        schema: String,
        nonce: [u8; 32],
    },
    ContactDescribed {
        schema: String,
        nonce: [u8; 32],
        encounter: Box<MemberEncounter>,
    },
    Advertise {
        schema: String,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
    },
    Resume {
        schema: String,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
    },
    CarrierChallenge {
        schema: String,
        challenge: Box<conduit_body::FederationChallenge>,
        signature: Vec<u8>,
    },
    CarrierProof {
        schema: String,
        challenge: Box<conduit_body::FederationChallenge>,
        signature: Vec<u8>,
    },
    CarrierAuthenticated {
        schema: String,
        inspection: Box<conduit_body::FederationInspection>,
    },
    Challenge {
        schema: String,
        challenge: Box<PartReturnChallenge>,
    },
    Proof {
        schema: String,
        proof: MemberReturnProofDocument,
    },
    Admitted {
        schema: String,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
    },
    Refused {
        schema: String,
        code: String,
    },
}
pub(crate) use conduit_body::MEMBER_RETURN_SCHEMA;

pub(crate) fn serve_body_member_route(
    state: &Path,
    credential_path: &Path,
    ttl_seconds: u64,
    bind: SocketAddr,
    public_url: &str,
    identity: (&Path, &Path),
    authorize: bool,
) -> Result<(), String> {
    let (certificate, private_key) = identity;
    if !authorize
        || !(1..=600).contains(&ttl_seconds)
        || !public_url.starts_with("wss://")
        || public_url.len() > 256
    {
        return Err("member route requires explicit finite pinned TLS authorization".into());
    }
    let selected: MembershipCredential =
        serde_json::from_slice(&bounded_read(credential_path, 64 * 1024)?)
            .map_err(|_| "invalid selected member credential")?;
    let listener = SecureWebSocketListener::bind(
        bind,
        certificate,
        private_key,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
        true,
    )
    .map_err(|e| format!("bind member route: {e:?}"))?;
    if listener
        .local_addr()
        .map_err(|e| format!("member route address: {e:?}"))?
        != bind
    {
        return Err("member route did not retain exact authorized bind".into());
    }
    let expiry = current_time_millis()?
        .checked_add(ttl_seconds * 1000)
        .ok_or("member route expiry overflow")?;
    let encounter = MemberEncounter {
        schema: MEMBER_ROUTE_SCHEMA.into(),
        body_id: selected.body_id,
        part_id: selected.part_id,
        host_id: selected.host_id,
        owner: crate::durable_host_control::carrier_receiver(state)?,
        candidate: RendezvousCandidate {
            candidate_id: "candidate/body-member-route".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: public_url.into(),
            authentication: RendezvousAuthentication {
                server_identity: route_server_identity(public_url)?,
                transport_binding_sha256: listener.certificate_binding_sha256(),
            },
            expires_at_millis: expiry,
            maximum_attempts: 2,
            attempt_timeout_millis: 10_000,
        },
    };
    println!(
        "{}",
        serde_json::to_string(&encounter).map_err(|e| e.to_string())?
    );
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(ttl_seconds);
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        if left.is_zero() {
            break;
        }
        let mut line = listener
            .accept_with_timeout(left)
            .map_err(|e| format!("member route accept: {e:?}"))?;
        line.set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("member route deadline: {e:?}"))?;
        let frame = match receive(&mut line) {
            Ok(frame) => frame,
            Err(error) => {
                refuse(&mut line, &error);
                continue;
            }
        };
        serve_member_frame(state, &encounter, &listener, line, frame)?;
    }
    Ok(())
}
/// One finite connection, shared by finite routes and an explicitly selected contact service.
pub(super) fn serve_member_frame(
    state: &Path,
    encounter: &MemberEncounter,
    listener: &SecureWebSocketListener,
    mut line: conduit_std_host::secure_websocket::SecureWebSocketLine,
    frame: MemberReturnFrame,
) -> Result<(), String> {
    let result = if matches!(&frame, MemberReturnFrame::Resume { .. }) {
        super::current_member::serve(state, encounter, &mut line, frame)
            .map(|(member, session)| (member, Some(session)))
    } else {
        exchange(state, encounter, &mut line, frame).map(|member| (member, None))
    };
    let (member, carrier) = match result {
        Ok(member) => member,
        Err(error) => {
            refuse(&mut line, &error);
            return Ok(());
        }
    };
    if let Some(session) = carrier {
        let result = crate::durable_host::owner::serve_member_presence(
            state,
            line,
            session.clone(),
            &member,
        );
        let cleanup = crate::durable_host_control::retire_carrier(state, session);
        if let Err(error) = result {
            eprintln!("member presence ended: {error}");
        }
        return cleanup;
    }
    if let Some(grant) =
        face_snapshot::serve_optional_face_snapshot(&mut line, listener, state, &member)?
    {
        return_route::serve(listener, state, &grant)?;
    }
    Ok(())
}

pub(super) fn refuse(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    error: &str,
) {
    let _ = send(
        line,
        &MemberReturnFrame::Refused {
            schema: MEMBER_RETURN_SCHEMA.into(),
            code: error.chars().take(256).collect(),
        },
    );
}

fn exchange(
    state: &Path,
    encounter: &MemberEncounter,
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    frame: MemberReturnFrame,
) -> Result<MemberPresentationBasis, String> {
    if current_time_millis()? >= encounter.candidate.expires_at_millis {
        return Err("member encounter expired".into());
    }
    let MemberReturnFrame::Advertise {
        schema,
        credential,
        advertisement,
    } = frame
    else {
        return Err("member return requires its retained credential and actual offer".into());
    };
    if schema != MEMBER_RETURN_SCHEMA
        || credential.body_id != encounter.body_id
        || credential.part_id != encounter.part_id
        || credential.host_id != encounter.host_id
        || advertisement.host_id != encounter.host_id
    {
        return Err("member return differs from authorized encounter scope".into());
    }
    let challenge = crate::durable_host_control::begin_member_return(
        state,
        credential,
        (*advertisement).clone(),
    )?;
    send(
        line,
        &MemberReturnFrame::Challenge {
            schema: MEMBER_RETURN_SCHEMA.into(),
            challenge: Box::new(challenge),
        },
    )?;
    let MemberReturnFrame::Proof { schema, proof } = receive(line)? else {
        return Err("member return requires continuity proof".into());
    };
    if schema != MEMBER_RETURN_SCHEMA {
        return Err("member return proof schema".into());
    }
    let credential = crate::durable_host_control::complete_member_return(state, proof)?;
    let member = MemberPresentationBasis::new(credential.clone(), (*advertisement).clone())?;
    send(
        line,
        &MemberReturnFrame::Admitted {
            schema: MEMBER_RETURN_SCHEMA.into(),
            credential,
            advertisement,
        },
    )?;
    Ok(member)
}

/// Explicit transport selection while the installed Host is stopped. This
/// descriptor is reachability and a TLS pin, never membership or a Show grant.
pub(crate) fn install_member_encounter(
    state: &Path,
    descriptor: &Path,
    authorize: bool,
) -> Result<(), String> {
    if !authorize {
        return Err("selecting a member encounter requires --authorize-route".into());
    }
    let _host = super::super::owner_lock::acquire(state)?;
    let installation = super::super::read_installation(&state.join("installation.json"))?;
    let credential = super::super::membership::load_joined_credential(state, &installation)?;
    let encounter: MemberEncounter = serde_json::from_slice(&bounded_read(descriptor, 16 * 1024)?)
        .map_err(|_| "invalid finite member encounter")?;
    let now = current_time_millis()?;
    let candidate = &encounter.candidate;
    if encounter.schema != MEMBER_ROUTE_SCHEMA
        || encounter.body_id != credential.body_id
        || encounter.part_id != credential.part_id
        || encounter.host_id != credential.host_id
        || encounter.owner.host_id.as_str().is_empty()
        || encounter.owner.host_id.as_str().len() > 256
        || encounter.owner.host_id == encounter.host_id
        || encounter.owner.boot_id.as_str().is_empty()
        || encounter.owner.boot_id.as_str().len() > 256
        || encounter.owner.offer_generation.0 == 0
        || encounter.owner.verifying_key == [0; 32]
        || candidate.line_family != RendezvousLineFamily::AuthenticatedTlsStream
        || candidate.expires_at_millis <= now
        || candidate.expires_at_millis.saturating_sub(now) > 600_000
        || !(1..=2).contains(&candidate.maximum_attempts)
        || !(1..=30_000).contains(&candidate.attempt_timeout_millis)
        || candidate.authentication.transport_binding_sha256 == [0; 32]
        || candidate.authentication.server_identity
            != route_server_identity(&candidate.reachability)?
    {
        return Err("member encounter differs from retained scope or finite TLS policy".into());
    }
    super::super::write_json_atomic(&state.join("body/member-encounter.json"), &encounter)
}

#[cfg(test)]
mod acknowledgement_wire_tests {
    use super::*;
    #[test]
    fn tls_admitted_frame_is_the_shared_acknowledgement_document() {
        let ad = conduit_std_host::StdHost::new().advertisement().clone();
        let credential:MembershipCredential=serde_json::from_value(serde_json::json!({
            "credential_id":"credential/1","body_id":"body/1","part_id":"part/1", "host_id":ad.host_id,"boot_id":ad.boot_id,"issued_at_millis":1
        })).unwrap();
        let frame = MemberReturnFrame::Admitted {
            schema: MEMBER_RETURN_SCHEMA.into(),
            credential: credential.clone(),
            advertisement: Box::new(ad.clone()),
        };
        let acknowledgement = conduit_body::MemberReturnAcknowledgement::new(credential, ad);
        assert_eq!(
            serde_json::to_vec(&frame).unwrap(),
            serde_json::to_vec(&acknowledgement).unwrap()
        );
    }
}
