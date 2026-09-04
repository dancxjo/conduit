use conduit_body::{BodyMembership, HostPresenceTable, MembershipCredential};
use conduit_core::{LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionSocket,
    BrowserAdmissionSocketError, BROWSER_ADMISSION_PROTOCOL,
};
use conduit_std_host::websocket::{NativeWebSocketError, NativeWebSocketError::Transport};
use std::io::ErrorKind;
use std::time::{Duration, Instant};

#[allow(clippy::too_many_arguments)]
pub(super) fn wait_for_return_close(
    socket: &mut BrowserAdmissionSocket,
    presence: &mut HostPresenceTable,
    membership: &mut BodyMembership,
    credential: &MembershipCredential,
    session: &LinkBindingId,
    clock: Instant,
    lease_millis: u64,
    renew_after_millis: u64,
) -> Result<(), String> {
    loop {
        let now = monotonic_millis(clock)?;
        let remaining = presence.leases[0].expires_at_millis.saturating_sub(now);
        socket
            .set_read_timeout(Some(Duration::from_millis(remaining.max(1))))
            .map_err(|error| format!("set returned presence deadline: {error:?}"))?;
        match socket.receive() {
            Ok(BrowserAdmissionIngress::PresenceRenewal {
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
                ..
            }) if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                let now = monotonic_millis(clock)?;
                presence
                    .renew(
                        membership,
                        &credential.part_id,
                        session,
                        sequence,
                        now,
                        lease_millis,
                        SignId::from(format!("sign/browser-admission-probe/returned-{sequence}")),
                    )
                    .map_err(|error| format!("renew returned presence: {error:?}"))?;
                socket
                    .send(&BrowserAdmissionEgress::PresenceAccepted {
                        protocol: BROWSER_ADMISSION_PROTOCOL,
                        sequence,
                        renew_after_millis,
                        expires_at_millis: presence.leases[0].expires_at_millis,
                    })
                    .map_err(|error| format!("accept returned renewal: {error:?}"))?;
                println!("returned-renewed sequence={sequence}");
            }
            Ok(BrowserAdmissionIngress::PresenceRenewal { .. }) => {
                return Err("returned renewal used a stale credential".into());
            }
            Ok(BrowserAdmissionIngress::PresenceLeave {
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
                ..
            }) if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                presence
                    .lose_session(
                        membership,
                        &credential.part_id,
                        session,
                        monotonic_millis(clock)?,
                        SignId::from("sign/browser-admission-probe/returned-explicit-leave"),
                    )
                    .map_err(|error| format!("record returned explicit leave: {error:?}"))?;
                println!("returned-left sequence={sequence}");
                return Ok(());
            }
            Ok(BrowserAdmissionIngress::PresenceLeave { .. }) => {
                return Err("returned leave used a stale credential".into());
            }
            Ok(_) => return Err("returned session frame was not a renewal".into()),
            Err(BrowserAdmissionSocketError::Transport(Transport(
                ErrorKind::TimedOut | ErrorKind::WouldBlock,
            ))) => {
                presence
                    .expire(
                        membership,
                        &credential.part_id,
                        monotonic_millis(clock)?,
                        SignId::from("sign/browser-admission-probe/returned-expired"),
                    )
                    .map_err(|error| format!("expire returned presence: {error:?}"))?;
                println!("returned-unavailable reason=expired");
                return Ok(());
            }
            Err(BrowserAdmissionSocketError::Transport(
                NativeWebSocketError::Disconnected | NativeWebSocketError::Transport(_),
            )) => {
                presence
                    .lose_session(
                        membership,
                        &credential.part_id,
                        session,
                        monotonic_millis(clock)?,
                        SignId::from("sign/browser-admission-probe/returned-session-lost"),
                    )
                    .map_err(|error| format!("lose returned session: {error:?}"))?;
                println!("returned-unavailable reason=session-lost");
                return Ok(());
            }
            Err(error) => return Err(format!("receive returned presence: {error:?}")),
        }
    }
}

fn monotonic_millis(clock: Instant) -> Result<u64, String> {
    u64::try_from(clock.elapsed().as_millis())
        .map_err(|_| "returned presence clock overflowed".into())
}
