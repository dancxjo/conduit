//! Reachability of an already authenticated member on its retained TLS carrier.
//! Uses the existing participant framing and Core presence leases. Presentation
//! remains refused until this entrance has actual native Line/route custody.
use super::{transport::Socket, In, Out, PROTOCOL, RENEW_MS};
use conduit_body::{HostPresenceLease, MembershipCredential};
use conduit_core::LinkBindingId;
use conduit_std_host::secure_websocket::SecureWebSocketLine;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(crate) fn serve_member_presence(
    state: &Path,
    line: SecureWebSocketLine,
    binding: LinkBindingId,
    credential: &MembershipCredential,
) -> Result<(), String> {
    serve_with_carrier(line, binding.clone(), credential, |sequence| {
        crate::durable_host_control::renew_carrier_presence(state, binding.clone(), sequence)
    })
}

pub(crate) fn serve_with_carrier(
    mut line: SecureWebSocketLine,
    binding: LinkBindingId,
    credential: &MembershipCredential,
    renew: impl FnMut(u64) -> Result<HostPresenceLease, String>,
) -> Result<(), String> {
    // Authentication has consumed and admitted the complete bounded request.
    line.complete_bounded_input();
    let mut socket = Socket::from_secure(line, binding)?;
    let result = serve_with(&mut socket, credential, renew);
    socket.close();
    result
}

pub(super) fn serve_with(
    socket: &mut Socket,
    credential: &MembershipCredential,
    mut renew: impl FnMut(u64) -> Result<HostPresenceLease, String>,
) -> Result<(), String> {
    let mut received_at = Instant::now();
    socket.replace_input_deadline(received_at + Duration::from_millis(super::LEASE_MS))?;
    let mut lease = renew(1)?;
    validate_lease(socket, credential, &lease, 1)?;
    let mut deadline = lease_deadline(received_at, &lease);
    socket.replace_input_deadline(deadline)?;
    acknowledge(socket, &lease)?;
    // The owner enforces its independently issued overall time/frame bounds on
    // every renewal. The worker also has a fixed input budget, including refused
    // frames; no peer-selected timeout or sequence can enlarge these bounds.
    for _ in 0..128 {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or("member presence lease expired")?;
        let (frame, _) = socket.receive(left)?;
        if Instant::now() >= deadline {
            return Err("member presence lease expired".into());
        }
        let (leave, sequence, matches) = match frame {
            In::PresenceRenewal {
                protocol,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
            } => (
                false,
                sequence,
                protocol == PROTOCOL
                    && credential_id == credential.credential_id
                    && body_id == credential.body_id
                    && part_id == credential.part_id
                    && host_id == credential.host_id
                    && boot_id == credential.boot_id,
            ),
            In::PresenceLeave {
                protocol,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
            } => (
                true,
                sequence,
                protocol == PROTOCOL
                    && credential_id == credential.credential_id
                    && body_id == credential.body_id
                    && part_id == credential.part_id
                    && host_id == credential.host_id
                    && boot_id == credential.boot_id,
            ),
            _ => return Err("member carrier has no selected presentation or action Line".into()),
        };
        if !matches || sequence <= lease.sequence {
            return Err("member presence credential or sequence differs".into());
        }
        if leave {
            return Ok(());
        }
        // Measure from before owner processing, so local control latency cannot
        // extend the independently enforced owner lease.
        received_at = Instant::now();
        lease = renew(sequence)?;
        validate_lease(socket, credential, &lease, sequence)?;
        deadline = lease_deadline(received_at, &lease);
        socket.replace_input_deadline(deadline)?;
        acknowledge(socket, &lease)?;
    }
    Err("member presence input budget exhausted".into())
}
fn acknowledge(socket: &mut Socket, lease: &HostPresenceLease) -> Result<(), String> {
    socket.send(&Out::PresenceAccepted {
        protocol: PROTOCOL,
        sequence: lease.sequence,
        renew_after_millis: RENEW_MS,
        expires_at_millis: lease.expires_at_millis,
    })
}

fn lease_deadline(received_at: Instant, lease: &HostPresenceLease) -> Instant {
    received_at
        + Duration::from_millis(
            lease
                .expires_at_millis
                .saturating_sub(lease.observed_at_millis),
        )
}

fn validate_lease(
    socket: &Socket,
    credential: &MembershipCredential,
    lease: &HostPresenceLease,
    sequence: u64,
) -> Result<(), String> {
    if lease.part_id != credential.part_id
        || lease.host_id != credential.host_id
        || lease.boot_id != credential.boot_id
        || lease.session_binding_id != *socket.binding()
        || lease.sequence != sequence
        || lease.state != conduit_body::HostPresenceState::Available
        || lease.expires_at_millis <= lease.observed_at_millis
        || lease.expires_at_millis - lease.observed_at_millis > super::LEASE_MS
    {
        return Err("owner presence reply differs from selected member carrier".into());
    }
    Ok(())
}
