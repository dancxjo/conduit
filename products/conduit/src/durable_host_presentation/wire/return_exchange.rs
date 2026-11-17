//! One authenticated signed member-return exchange on its original I/O deadline.
use super::{connect, receive, send, ADMISSION, MESSAGE};
use conduit_body::RendezvousCandidate;
use conduit_std_host::secure_websocket::SecureWebSocketClientLine;
use std::time::{Duration, Instant};

pub(super) fn signed_return_exchange(
    state: &std::path::Path,
    prior: &conduit_body::MembershipCredential,
    actual: &conduit_core::HostAdvertisement,
    candidate: &RendezvousCandidate,
) -> Result<
    (
        SecureWebSocketClientLine,
        conduit_body::MembershipCredential,
    ),
    String,
> {
    use crate::durable_host::routed_join::{MemberReturnFrame, MEMBER_RETURN_SCHEMA};
    let deadline = Instant::now()
        + Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000)));
    let mut line = connect(
        candidate,
        Duration::from_millis(u64::from(candidate.attempt_timeout_millis.min(3_000))),
    )?;
    line.replace_io_deadline(deadline)
        .map_err(|e| format!("member return I/O deadline:{e:?}"))?;
    send(
        &mut line,
        &MemberReturnFrame::Advertise {
            schema: MEMBER_RETURN_SCHEMA.into(),
            credential: prior.clone(),
            advertisement: Box::new(actual.clone()),
        },
        ADMISSION,
    )?;
    let challenge = match receive::<MemberReturnFrame>(&mut line, MESSAGE)? {
        MemberReturnFrame::Challenge { schema, challenge } if schema == MEMBER_RETURN_SCHEMA => {
            *challenge
        }
        MemberReturnFrame::Refused { code, .. } => {
            return Err(format!("member return refused: {code}"));
        }
        _ => return Err("member return challenge response schema".into()),
    };
    let now_millis = super::return_validity::wait_until_valid(
        challenge.issued_at_millis,
        challenge.expires_at_millis,
        conduit_body::MAX_ADMISSION_TTL_MILLIS,
        deadline,
        || {
            let unix_millis = crate::durable_host::current_time_millis()
                .map_err(|_| super::return_validity::NotBeforeRefusal::ClockUnavailable)?;
            Ok(super::return_validity::ClockSample {
                unix_millis,
                monotonic: Instant::now(),
            })
        },
        |duration| {
            std::thread::sleep(duration);
            Ok(())
        },
    )
    .map_err(|why| format!("member return challenge validity: {why:?}"))?;
    let proof = crate::durable_host::membership::sign_return_challenge(
        state, prior, actual, &challenge, now_millis,
    )?;
    send(
        &mut line,
        &MemberReturnFrame::Proof {
            schema: MEMBER_RETURN_SCHEMA.into(),
            proof: crate::durable_host::owner::MemberReturnProofDocument::encode(proof),
        },
        MESSAGE,
    )?;
    let credential = match receive::<MemberReturnFrame>(&mut line, ADMISSION)? {
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
            return Err(format!("member return refused: {code}"));
        }
        _ => return Err("member return response differs from authenticated challenge".into()),
    };
    Ok((line, credential))
}
