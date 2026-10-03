//! One explicitly authorized browser admission window while the Body is lulled.
//! The window bounds admission authorization, not carrier handshake/close latency.
//! This is membership and presence only, never admission of remote execution.
mod admission;
mod service;
#[cfg(unix)]
mod service_worker;
mod transport;
use super::Owner;
use conduit_body::{BodyState, HostPresenceClock, HostPresenceClockScale, HostPresenceTable};
use conduit_core::{HostId, LinkBindingId, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BrowserAdmissionIngress as In,
    BROWSER_ADMISSION_PROTOCOL as PROTOCOL,
};
pub(crate) use service::{BrowserAdmittedSnapshot, BrowserWindow, BrowserWindowAuthorization};
#[cfg(unix)]
pub(crate) use service_worker::run_service_window;
use std::{
    path::Path,
    time::{Duration, Instant},
};
const LEASE_MS: u64 = 2000;
const RENEW_MS: u64 = 500;
const MAX_CONNECTIONS: usize = 8;

impl Owner {
    pub(crate) fn admit_browser(
        &mut self,
        root: &Path,
        expected_host: &str,
        new_host_key: Option<[u8; 32]>,
        maximum_millis: u64,
    ) -> Result<(), String> {
        if self.session.evidence().body.state != BodyState::Lulled
            || self.session.realization().is_some()
        {
            return Err(
                "browser admission requires a lulled Body without a pending realization".into(),
            );
        }
        if !(1000..=60_000).contains(&maximum_millis)
            || expected_host.is_empty()
            || expected_host.len() > 256
        {
            return Err(
                "browser admission requires an exact Host and a window of 1000..60000 ms".into(),
            );
        }
        let expected = HostId::from(expected_host);
        if expected == self.host.advertisement().host_id {
            return Err("browser participant cannot replace the owner Host".into());
        }
        let known = self.admissions.as_ref().is_some_and(|manager| {
            manager
                .receipts
                .iter()
                .any(|receipt| receipt.credential.host_id == expected)
        });
        if !known && new_host_key.is_none() {
            return Err("first browser admission requires its exact public verifying key".into());
        }
        let listener = transport::Listener::bind()?;
        let clock = Instant::now();
        let deadline = clock + Duration::from_millis(maximum_millis);
        super::super::emit(&serde_json::json!({
            "schema":"conduit.body/browser-admission-window@1", "body_id":self.session.evidence().body_id,
            "expected_host_id":expected, "url":listener.url()?, "maximum_millis":maximum_millis,
            "maximum_connections":MAX_CONNECTIONS, "remote_execution":false,
            "window_scope":"admission-authorization", "strict_elapsed_termination":false,
        }))?;
        for _ in 0..MAX_CONNECTIONS {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let Some(mut socket) = listener.accept(remaining)? else {
                break;
            };
            let binding = LinkBindingId::from(format!(
                "line/owner-browser/{}",
                nonce()?
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            ));
            let result = (|| {
                let (credential, observation) = admission::admit(
                    self,
                    &mut socket,
                    &expected,
                    new_host_key,
                    &binding,
                    clock,
                    deadline,
                )?;
                admission::remaining(deadline)?;
                self.persist(root)?; // A credential is never acknowledged ahead of durable membership.
                let snapshot =
                    BrowserAdmittedSnapshot::from_foreground(self, credential, observation)?;
                serve_presence(&snapshot, &mut socket, &binding, clock, deadline)
            })();
            // Every exit fences the current browser incarnation, including failed acknowledgement.
            let current = self
                .session
                .evidence()
                .membership
                .parts
                .iter()
                .filter_map(|part| part.current.as_ref())
                .find(|h| h.host_id == expected)
                .cloned();
            if let Some(current) = current {
                let authority = self.host.advertisement();
                self.session
                    .observe_host_lost(
                        &current.host_id,
                        &current.boot_id,
                        &authority.host_id,
                        &authority.boot_id,
                    )
                    .map_err(debug)?;
                self.persist(root)?;
                let _ = socket.send(&Out::BiographyEvidence {
                    protocol: PROTOCOL,
                    evidence: Box::new(self.session.evidence().clone()),
                });
            }
            socket.close();
            super::super::emit(&serde_json::json!({
                "schema":"conduit.body/browser-session-ended@1", "result":result,
                "truth":self.truth(), "remote_execution":false,
            }))?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn serve_presence(
    snapshot: &BrowserAdmittedSnapshot,
    socket: &mut transport::Socket,
    binding: &LinkBindingId,
    clock: Instant,
    deadline: Instant,
) -> Result<&'static str, String> {
    let credential = &snapshot.credential;
    let presence_clock = HostPresenceClock::new(
        binding.as_str().into(),
        HostPresenceClockScale::Milliseconds,
        1,
        1,
    )
    .map_err(debug)?;
    let mut presence = HostPresenceTable::new(credential.body_id.clone(), presence_clock, LEASE_MS)
        .map_err(debug)?;
    let membership = &snapshot.biography.membership;
    presence
        .start(
            membership,
            &credential.part_id,
            binding.clone(),
            1,
            now(clock),
            LEASE_MS,
            signal(binding, "started"),
        )
        .map_err(debug)?;
    admission::remaining(deadline)?;
    socket.send(&Out::Admitted {
        protocol: PROTOCOL,
        credential: credential.clone(),
    })?;
    socket.send(&Out::BiographyEvidence {
        protocol: PROTOCOL,
        evidence: snapshot.biography.clone(),
    })?;
    socket.send(&Out::OfferEvidence {
        protocol: PROTOCOL,
        evidence: snapshot.offer.clone(),
    })?;
    acknowledge_presence(socket, &presence)?;
    for _ in 0..256 {
        let lease = &presence.leases[0];
        let Some(window_left) = deadline.checked_duration_since(Instant::now()) else {
            return Ok("window-closed");
        };
        let remaining = lease.expires_at_millis.saturating_sub(now(clock));
        if remaining == 0 {
            return Err("browser presence lease expired".into());
        }
        let received = socket.receive(window_left.min(Duration::from_millis(remaining)));
        if Instant::now() >= deadline {
            return Ok("window-closed");
        }
        if now(clock) >= lease.expires_at_millis {
            return Err("browser presence lease expired".into());
        }
        let (frame, _) = received?;
        match frame {
            In::PresenceRenewal {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
            } if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                presence
                    .renew(
                        membership,
                        &part_id,
                        binding,
                        sequence,
                        now(clock),
                        LEASE_MS,
                        signal(binding, &format!("renew-{sequence}")),
                    )
                    .map_err(debug)?;
                acknowledge_presence(socket, &presence)?;
            }
            In::WebRtcGrantRequest {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                generation,
                index,
            } if credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                // Membership supplies no planned remote Line or execution authority.
                socket.send(&Out::WebRtcGrant {
                    protocol: PROTOCOL,
                    generation,
                    index,
                    total: 0,
                    grant: None,
                })?;
            }
            In::PresenceLeave {
                protocol: PROTOCOL,
                credential_id,
                body_id,
                part_id,
                host_id,
                boot_id,
                sequence,
            } if sequence > lease.sequence
                && credential_id == credential.credential_id
                && body_id == credential.body_id
                && part_id == credential.part_id
                && host_id == credential.host_id
                && boot_id == credential.boot_id =>
            {
                return Ok("browser-left")
            }
            _ => return Err("browser session refused unexpected or stale credential frame".into()),
        }
    }
    Err("browser presence frame budget exhausted".into())
}
fn acknowledge_presence(
    socket: &mut transport::Socket,
    presence: &HostPresenceTable,
) -> Result<(), String> {
    let lease = &presence.leases[0];
    socket.send(&Out::PresenceAccepted {
        protocol: PROTOCOL,
        sequence: lease.sequence,
        renew_after_millis: RENEW_MS,
        expires_at_millis: lease.expires_at_millis,
    })
}
fn now(clock: Instant) -> u64 {
    clock.elapsed().as_millis() as u64
}
fn nonce() -> Result<[u8; 32], String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
fn signal(binding: &LinkBindingId, stage: &str) -> SignId {
    SignId::from(format!("sign/{}/{stage}", binding.as_str()))
}
fn debug(error: impl std::fmt::Debug) -> String {
    format!("browser participant: {error:?}")
}
