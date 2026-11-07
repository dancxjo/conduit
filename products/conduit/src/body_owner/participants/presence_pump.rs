//! Shared finite carrier receive and owner-authorized presence renewal.
use super::{transport::Socket, In, Out, PROTOCOL, RENEW_MS};
use conduit_body::{HostPresenceLease, HostPresenceState, MembershipCredential};
use std::time::{Duration, Instant};

#[allow(
    clippy::large_enum_variant,
    reason = "Keep the bounded decoded frame inline without a per-frame allocation."
)]
pub(super) enum PresenceEvent {
    Continue,
    Leave,
    Other(In),
}

pub(super) struct PresencePump {
    credential: MembershipCredential,
    lease: HostPresenceLease,
    lease_deadline: Instant,
    overall_deadline: Instant,
    remaining_frames: usize,
    absolute_tls: bool,
}

impl PresencePump {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start(
        socket: &mut Socket,
        credential: &MembershipCredential,
        initial_lease: HostPresenceLease,
        anchor: Instant,
        overall_deadline: Instant,
        max_frames: usize,
        absolute_tls: bool,
    ) -> Result<Self, String> {
        validate_lease(socket, credential, &initial_lease, initial_lease.sequence)?;
        let lease_deadline = lease_deadline(anchor, &initial_lease)?;
        let pump = Self {
            credential: credential.clone(),
            lease: initial_lease,
            lease_deadline,
            overall_deadline,
            remaining_frames: max_frames,
            absolute_tls,
        };
        pump.check_lease()?;
        if Instant::now() >= overall_deadline {
            return Err("participant presence window closed".into());
        }
        if absolute_tls {
            socket.replace_input_deadline(lease_deadline.min(overall_deadline))?;
        }
        pump.acknowledge(socket)?;
        Ok(pump)
    }

    pub(super) fn wait_remaining(&self) -> Duration {
        self.lease_deadline
            .min(self.overall_deadline)
            .saturating_duration_since(Instant::now())
    }

    fn check_lease(&self) -> Result<(), String> {
        if Instant::now() >= self.lease_deadline {
            return Err("participant presence lease expired".into());
        }
        Ok(())
    }

    pub(super) fn receive(
        &mut self,
        socket: &mut Socket,
        poll_hint: Option<Duration>,
        deferred: Option<(In, u32)>,
    ) -> Result<Option<(In, u32)>, String> {
        if Instant::now() >= self.overall_deadline {
            return Ok(None);
        }
        self.check_lease()?;
        let received = if let Some(frame) = deferred {
            // Deferred frames were counted when actually read from the carrier.
            Some(frame)
        } else {
            if self.remaining_frames == 0 {
                return Err("participant presence frame budget exhausted".into());
            }
            let wait = self.wait_remaining();
            let frame = if let Some(hint) = poll_hint {
                socket.receive_polled(wait.min(hint))?
            } else {
                Some(socket.receive(wait)?)
            };
            if frame.is_some() {
                self.remaining_frames -= 1;
            }
            frame
        };
        if Instant::now() >= self.overall_deadline {
            return Ok(None);
        }
        self.check_lease()?;
        Ok(received)
    }

    pub(super) fn handle_presence(
        &mut self,
        socket: &mut Socket,
        frame: In,
        mut renew: impl FnMut(u64) -> Result<HostPresenceLease, String>,
    ) -> Result<PresenceEvent, String> {
        self.check_lease()?;
        if Instant::now() >= self.overall_deadline {
            return Err("participant presence window closed".into());
        }
        let (leave, protocol, credential_id, body_id, part_id, host_id, boot_id, sequence) =
            match frame {
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
                    protocol,
                    credential_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    sequence,
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
                    protocol,
                    credential_id,
                    body_id,
                    part_id,
                    host_id,
                    boot_id,
                    sequence,
                ),
                other => return Ok(PresenceEvent::Other(other)),
            };
        let credential = &self.credential;
        if protocol != PROTOCOL
            || credential_id != credential.credential_id
            || body_id != credential.body_id
            || part_id != credential.part_id
            || host_id != credential.host_id
            || boot_id != credential.boot_id
            || sequence <= self.lease.sequence
        {
            return Err("participant presence credential or sequence differs".into());
        }
        if leave {
            return Ok(PresenceEvent::Leave);
        }
        // Owner processing latency cannot enlarge the independently issued lease.
        let anchor = Instant::now();
        let lease = renew(sequence)?;
        validate_lease(socket, credential, &lease, sequence)?;
        if lease.offer_generation != self.lease.offer_generation
            || lease.membership_proof_id != self.lease.membership_proof_id
        {
            return Err("owner presence reply changed its admitted incarnation".into());
        }
        let deadline = lease_deadline(anchor, &lease)?;
        if Instant::now() >= self.overall_deadline || Instant::now() >= deadline {
            return Err("participant presence renewal expired during owner processing".into());
        }
        if self.absolute_tls {
            socket.replace_input_deadline(deadline.min(self.overall_deadline))?;
        }
        self.lease = lease;
        self.lease_deadline = deadline;
        self.acknowledge(socket)?;
        Ok(PresenceEvent::Continue)
    }

    fn acknowledge(&self, socket: &mut Socket) -> Result<(), String> {
        socket.send(&Out::PresenceAccepted {
            protocol: PROTOCOL,
            sequence: self.lease.sequence,
            renew_after_millis: RENEW_MS,
            expires_at_millis: self.lease.expires_at_millis,
        })
    }
}

fn lease_deadline(anchor: Instant, lease: &HostPresenceLease) -> Result<Instant, String> {
    anchor
        .checked_add(Duration::from_millis(
            lease.expires_at_millis - lease.observed_at_millis,
        ))
        .ok_or_else(|| "participant presence deadline overflow".into())
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
        || sequence == 0
        || lease.state != HostPresenceState::Available
        || lease.expires_at_millis <= lease.observed_at_millis
        || lease.expires_at_millis - lease.observed_at_millis > super::LEASE_MS
    {
        return Err("owner presence reply differs from selected participant carrier".into());
    }
    Ok(())
}
