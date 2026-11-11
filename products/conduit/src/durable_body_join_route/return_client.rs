//! Explicit member return using the running Host's actual Boot and retained key.
use super::member_return::{
    MemberEncounter, MemberReturnFrame, MEMBER_RETURN_SCHEMA, MEMBER_ROUTE_SCHEMA,
};
use super::*;
use conduit_body::MembershipCredential;
use conduit_core::HostAdvertisement;

pub(crate) fn return_installed_member(
    state: &Path,
    descriptor: &Path,
    authorize: bool,
) -> Result<(), String> {
    if !authorize {
        return Err("returning a member requires --authorize-return".into());
    }
    let encounter: MemberEncounter = serde_json::from_slice(&bounded_read(descriptor, 16 * 1024)?)
        .map_err(|_| "invalid bounded member encounter")?;
    let returned = crate::durable_host_control::return_member(state, encounter)?;
    println!(
        "{}",
        serde_json::to_string(&returned).map_err(|e| e.to_string())?
    );
    Ok(())
}

/// Called by the authenticated actor while its installed service locks are held.
pub(crate) fn return_on_actual_host(
    state: &Path,
    actual: &HostAdvertisement,
    encounter: &MemberEncounter,
) -> Result<MembershipCredential, String> {
    let installation = super::super::read_installation(&state.join("installation.json"))?;
    let prior = super::super::membership::load_joined_credential(state, &installation)?;
    let returned = return_over_tls(state, &prior, actual, encounter)?;
    super::super::membership::retain_returned_membership(state, &prior, &returned, actual)?;
    Ok(returned)
}

fn return_over_tls(
    state: &Path,
    prior: &MembershipCredential,
    actual: &HostAdvertisement,
    encounter: &MemberEncounter,
) -> Result<MembershipCredential, String> {
    let candidate = &encounter.candidate;
    if encounter.schema != MEMBER_ROUTE_SCHEMA
        || encounter.body_id != prior.body_id
        || encounter.part_id != prior.part_id
        || encounter.host_id != prior.host_id
        || actual.host_id != prior.host_id
        || candidate.line_family != RendezvousLineFamily::AuthenticatedTlsStream
        || candidate.expires_at_millis <= current_time_millis()?
        || candidate
            .expires_at_millis
            .saturating_sub(current_time_millis()?)
            > 600_000
        || candidate.authentication.transport_binding_sha256 == [0; 32]
        || candidate.authentication.server_identity
            != route_server_identity(&candidate.reachability)?
        || !(1..=30_000).contains(&candidate.attempt_timeout_millis)
        || !(1..=2).contains(&candidate.maximum_attempts)
    {
        return Err(
            "member return descriptor differs from retained Body/Part/Host or finite TLS route"
                .into(),
        );
    }
    let mut result = Err("member return had no authenticated exchange".into());
    for _ in 0..candidate.maximum_attempts {
        if current_time_millis()? >= candidate.expires_at_millis {
            return Err("member encounter expired".into());
        }
        result = exchange(state, prior, actual, candidate);
        if result.is_ok() {
            break;
        }
    }
    result
}

fn exchange(
    state: &Path,
    prior: &MembershipCredential,
    actual: &HostAdvertisement,
    candidate: &RendezvousCandidate,
) -> Result<MembershipCredential, String> {
    let timeout = Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000)));
    let mut line = SecureWebSocketClientLine::connect_pinned_for_exchange(
        route_address(&candidate.reachability)?,
        &candidate.reachability,
        &candidate.authentication.server_identity,
        candidate.authentication.transport_binding_sha256,
        timeout,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
    )
    .map_err(|e| format!("member return TLS:{e:?}"))?;
    line.set_read_timeout(Some(timeout))
        .map_err(|e| format!("member return timeout:{e:?}"))?;
    send(
        &mut line,
        &MemberReturnFrame::Advertise {
            schema: MEMBER_RETURN_SCHEMA.into(),
            credential: prior.clone(),
            advertisement: Box::new(actual.clone()),
        },
    )?;
    let challenge = match receive(&mut line)? {
        MemberReturnFrame::Challenge { schema, challenge } if schema == MEMBER_RETURN_SCHEMA => {
            *challenge
        }
        MemberReturnFrame::Refused { code, .. } => {
            return Err(format!("member return refused:{code}"))
        }
        _ => return Err("member return challenge response schema".into()),
    };
    let proof = super::super::membership::sign_return_challenge(
        state,
        prior,
        actual,
        &challenge,
        current_time_millis()?,
    )?;
    send(
        &mut line,
        &MemberReturnFrame::Proof {
            schema: MEMBER_RETURN_SCHEMA.into(),
            proof: crate::durable_host::owner::MemberReturnProofDocument::encode(proof),
        },
    )?;
    let returned = match receive(&mut line)? {
        MemberReturnFrame::Admitted {
            schema,
            credential,
            advertisement,
        } if schema == MEMBER_RETURN_SCHEMA
            && *advertisement == *actual
            && credential.issued_at_millis >= challenge.issued_at_millis
            && credential.issued_at_millis < challenge.expires_at_millis =>
        {
            credential
        }
        MemberReturnFrame::Refused { code, .. } => {
            return Err(format!("member return refused:{code}"))
        }
        _ => return Err("member return response differs from authenticated challenge".into()),
    };
    // The authenticated receipt is known even if closing this old carrier fails.
    let _ = line.close();
    Ok(returned)
}

fn send(line: &mut SecureWebSocketClientLine, frame: &MemberReturnFrame) -> Result<(), String> {
    let bytes = serde_json::to_vec(frame).map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_ROUTE_FRAME_BYTES {
        return Err("member return frame pressure".into());
    }
    line.send_binary(&bytes)
        .map_err(|e| format!("member return send:{e:?}"))
}
fn receive(line: &mut SecureWebSocketClientLine) -> Result<MemberReturnFrame, String> {
    let mut bytes = vec![0; MAXIMUM_ROUTE_FRAME_BYTES];
    let n = line
        .receive_binary(&mut bytes)
        .map_err(|e| format!("member return input:{e:?}"))?;
    serde_json::from_slice(&bytes[..n]).map_err(|_| "invalid member return response".into())
}

#[cfg(test)]
#[path = "return_client/tests.rs"]
mod tests;
