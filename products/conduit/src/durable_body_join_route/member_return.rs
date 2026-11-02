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
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(crate) enum MemberReturnFrame {
    Advertise {
        schema: String,
        credential: MembershipCredential,
        advertisement: Box<HostAdvertisement>,
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
pub(crate) const MEMBER_RETURN_SCHEMA: &str = "conduit.body/member-return@1";

// Keep the selected member and explicit TLS authorization inputs at this CLI boundary.
#[allow(clippy::too_many_arguments)]
pub(crate) fn serve_body_member_route(
    state: &Path,
    credential_path: &Path,
    ttl_seconds: u64,
    bind: SocketAddr,
    public_url: &str,
    certificate: &Path,
    private_key: &Path,
    authorize: bool,
) -> Result<(), String> {
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
        let result = exchange(state, &encounter, &mut line);
        let member = match result {
            Ok(member) => member,
            Err(error) => {
                let _ = send(
                    &mut line,
                    &MemberReturnFrame::Refused {
                        schema: MEMBER_RETURN_SCHEMA.into(),
                        code: error.chars().take(256).collect(),
                    },
                );
                continue;
            }
        };
        // Successful return changes membership only; this entrance grants no Mask or Line.
        drop(member);
        let _ = line.close();
    }
    Ok(())
}
fn exchange(
    state: &Path,
    encounter: &MemberEncounter,
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
) -> Result<MemberPresentationBasis, String> {
    if current_time_millis()? >= encounter.candidate.expires_at_millis {
        return Err("member encounter expired".into());
    }
    let MemberReturnFrame::Advertise {
        schema,
        credential,
        advertisement,
    } = receive(line)?
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
