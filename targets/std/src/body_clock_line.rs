//! Bounded authenticated Body clock exchange over an already established line.

use conduit_body::{
    BodyClockAdmissionRefusal, BodyClockPeerAdmission, BodyClockSourcePolicy, BodyId,
    BodyMembership, MembershipState, PartId,
};
use conduit_core::{
    BodyClockCorrelation, BodyClockRateEstimator, BodyTimeEstimate, BodyTimeRefusal,
    BodyTimeTracker, BootId, HostId, MonotonicInstant, PeerClockExchange,
};
use conduit_protected_line::{
    establish_protected_session, CarrierFailure, ProtectedFrameCarrier, ProtectedLineError,
    ProtectedSession, ProtectedSessionPolicy, Role, SessionBinding, SessionDisposition,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use crate::TimerAdapter;

const MAX_PAYLOAD: usize = 2048;
const WIRE_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyClockLineRefusal {
    WrongSession,
    Closed,
    Admission(BodyClockAdmissionRefusal),
    Transport(ProtectedLineError),
    NoClock,
    WrongClock,
    Malformed,
    Unexpected,
    Stale,
    Io(std::io::ErrorKind),
    Entropy,
}

pub fn establish_body_clock_tcp_session(
    stream: &mut TcpStream,
    role: Role,
    binding: &SessionBinding,
    policy: ProtectedSessionPolicy,
    preshared_key: [u8; 32],
) -> Result<ProtectedSession, BodyClockLineRefusal> {
    let mut ephemeral_private_key = [0; 32];
    getrandom::fill(&mut ephemeral_private_key).map_err(|_| BodyClockLineRefusal::Entropy)?;
    let mut carrier = TcpClockCarrier {
        stream,
        timeout: Duration::from_millis(u64::from(policy.handshake_timeout_millis)),
    };
    establish_protected_session(
        &mut carrier,
        role,
        binding,
        policy,
        preshared_key,
        ephemeral_private_key,
    )
    .map_err(BodyClockLineRefusal::Transport)
}

struct TcpClockCarrier<'a> {
    stream: &'a mut TcpStream,
    timeout: Duration,
}

impl ProtectedFrameCarrier for TcpClockCarrier<'_> {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        let deadline = Instant::now() + self.timeout;
        write_frame(self.stream, frame, deadline).map_err(carrier_failure)
    }

    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> Result<usize, CarrierFailure> {
        let deadline = Instant::now()
            + self
                .timeout
                .min(Duration::from_millis(u64::from(timeout_millis)));
        let frame = read_frame(self.stream, output.len(), deadline).map_err(carrier_failure)?;
        output[..frame.len()].copy_from_slice(&frame);
        Ok(frame.len())
    }

    fn close(&mut self) -> Result<(), CarrierFailure> {
        self.stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(|_| CarrierFailure::Lost)
    }
}

fn carrier_failure(error: BodyClockLineRefusal) -> CarrierFailure {
    match error {
        BodyClockLineRefusal::Io(std::io::ErrorKind::TimedOut) => CarrierFailure::TimedOut,
        BodyClockLineRefusal::Malformed => CarrierFailure::Pressure,
        _ => CarrierFailure::Lost,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum ClockFrame {
    Request {
        version: u8,
        body: String,
        sequence: u64,
        send: MonotonicInstant,
    },
    Response {
        version: u8,
        body: String,
        sequence: u64,
        receive: MonotonicInstant,
        send: MonotonicInstant,
        correlation: Box<BodyClockCorrelation>,
    },
}

pub struct BodyClockLine {
    binding: SessionBinding,
    role: Role,
    body: BodyId,
    local_host: HostId,
    local_boot: BootId,
    peer_host: HostId,
    peer_boot: BootId,
    part: PartId,
    policy: BodyClockSourcePolicy,
    membership_revision: conduit_body::BodyMembershipRevision,
    pending: Option<(u64, MonotonicInstant)>,
    last_sequence: u64,
}

impl BodyClockLine {
    #[allow(clippy::too_many_arguments)]
    pub fn exchange_tcp<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        stream: &mut TcpStream,
        generation: u64,
        tracker: &mut BodyTimeTracker,
        estimator: &mut BodyClockRateEstimator,
    ) -> Result<BodyTimeEstimate, BodyClockLineRefusal> {
        let response = self.request_tcp(membership, policy, session, timer, stream)?;
        self.receive_response(
            membership, policy, session, timer, &response, generation, tracker, estimator,
        )
    }

    pub fn bootstrap_tcp<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        stream: &mut TcpStream,
        generation: u64,
    ) -> Result<(BodyTimeTracker, BodyClockRateEstimator), BodyClockLineRefusal> {
        let response = self.request_tcp(membership, policy, session, timer, stream)?;
        self.receive_initial_response(membership, policy, session, timer, &response, generation)
    }

    fn request_tcp<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        stream: &mut TcpStream,
    ) -> Result<Vec<u8>, BodyClockLineRefusal> {
        let deadline = configure_tcp(stream, policy)?;
        let mut request = vec![0; session.maximum_frame_bytes()];
        let length = self.send_request(membership, policy, session, timer, &mut request)?;
        if let Err(error) = write_frame(stream, &request[..length], deadline) {
            session.close();
            return Err(error);
        }
        let response = match read_frame(stream, session.maximum_frame_bytes(), deadline) {
            Ok(response) => response,
            Err(error) => {
                session.close();
                return Err(error);
            }
        };
        Ok(response)
    }

    pub fn respond_tcp<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        stream: &mut TcpStream,
        local_correlation: &BodyClockCorrelation,
    ) -> Result<(), BodyClockLineRefusal> {
        let deadline = configure_tcp(stream, policy)?;
        let request = match read_frame(stream, session.maximum_frame_bytes(), deadline) {
            Ok(request) => request,
            Err(error) => {
                session.close();
                return Err(error);
            }
        };
        let mut response = vec![0; session.maximum_frame_bytes()];
        let length = self.respond(
            membership,
            policy,
            session,
            timer,
            &request,
            local_correlation,
            &mut response,
        )?;
        if let Err(error) = write_frame(stream, &response[..length], deadline) {
            session.close();
            return Err(error);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn admit(
        session: &ProtectedSession,
        role: Role,
        body: BodyId,
        local_host: HostId,
        local_boot: BootId,
        peer_host: HostId,
        peer_boot: BootId,
        part: PartId,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
    ) -> Result<Self, BodyClockLineRefusal> {
        let binding = session.evidence().binding.clone();
        let line = Self {
            binding,
            role,
            body,
            local_host,
            local_boot,
            peer_host,
            peer_boot,
            part,
            policy: policy.clone(),
            membership_revision: membership.revision,
            pending: None,
            last_sequence: 0,
        };
        line.check(session, membership, policy)?;
        Ok(line)
    }

    pub fn send_request<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        output: &mut [u8],
    ) -> Result<usize, BodyClockLineRefusal> {
        if self.role != Role::Initiator || self.pending.is_some() {
            return Err(BodyClockLineRefusal::Unexpected);
        }
        self.check(session, membership, policy)?;
        let sequence = self
            .last_sequence
            .checked_add(1)
            .ok_or(BodyClockLineRefusal::Stale)?;
        let send = self.sample(timer)?;
        let frame = ClockFrame::Request {
            version: WIRE_VERSION,
            body: self.body.as_str().into(),
            sequence,
            send: send.clone(),
        };
        let length = seal(session, &frame, output)?;
        self.pending = Some((sequence, send));
        Ok(length)
    }

    pub fn expire_pending<T: TimerAdapter>(
        &mut self,
        timer: &mut T,
    ) -> Result<bool, BodyClockLineRefusal> {
        let Some((sequence, sent)) = &self.pending else {
            return Ok(false);
        };
        let now = self.sample(timer)?;
        if now.clock() != sent.clock()
            || now.ticks() < sent.ticks()
            || now.clock().scale() != self.policy.maximum_round_trip.scale()
        {
            return Err(BodyClockLineRefusal::WrongClock);
        }
        if now.ticks() - sent.ticks() <= self.policy.maximum_round_trip.ticks() {
            return Ok(false);
        }
        self.last_sequence = *sequence;
        self.pending = None;
        Ok(true)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn respond<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        encrypted: &[u8],
        local_correlation: &BodyClockCorrelation,
        output: &mut [u8],
    ) -> Result<usize, BodyClockLineRefusal> {
        if self.role != Role::Responder {
            return Err(BodyClockLineRefusal::Unexpected);
        }
        self.check(session, membership, policy)?;
        let payload = open(session, encrypted)?;
        let receive = self.sample(timer)?;
        let request = decode(&payload)?;
        let ClockFrame::Request {
            version,
            body,
            sequence,
            send,
        } = request
        else {
            return Err(BodyClockLineRefusal::Unexpected);
        };
        if version != WIRE_VERSION
            || body != self.body.as_str()
            || sequence <= self.last_sequence
            || sequence == 0
            || sequence > session.limits().maximum_frames_per_direction
            || send.clock().host_id() != &self.peer_host
            || send.clock().boot_id() != &self.peer_boot
            || send.validate().is_err()
        {
            return Err(BodyClockLineRefusal::Stale);
        }
        if local_correlation.validate().is_err()
            || local_correlation.body_basis() != self.body.as_str()
            || local_correlation.generation() < policy.minimum_generation
            || local_correlation.local_clock() != receive.clock()
            || local_correlation.project(&receive).is_err()
        {
            return Err(BodyClockLineRefusal::WrongClock);
        }
        self.check(session, membership, policy)?;
        let send = self.sample(timer)?;
        if send.clock() != receive.clock()
            || send.ticks() < receive.ticks()
            || local_correlation.project(&send).is_err()
        {
            return Err(BodyClockLineRefusal::WrongClock);
        }
        let response = ClockFrame::Response {
            version: WIRE_VERSION,
            body,
            sequence,
            receive,
            send,
            correlation: Box::new(local_correlation.clone()),
        };
        let length = seal(session, &response, output)?;
        self.last_sequence = sequence;
        Ok(length)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn receive_response<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        encrypted: &[u8],
        generation: u64,
        tracker: &mut BodyTimeTracker,
        estimator: &mut BodyClockRateEstimator,
    ) -> Result<BodyTimeEstimate, BodyClockLineRefusal> {
        let (admission, exchange, correlation) =
            self.receive_exchange(membership, policy, session, timer, encrypted)?;
        let estimate = admission
            .reconcile_with_rate_estimator(
                membership,
                policy,
                &self.peer_host,
                &self.peer_boot,
                &exchange,
                &correlation,
                generation,
                tracker,
                estimator,
            )
            .map_err(BodyClockLineRefusal::Admission)?
            .clone();
        Ok(estimate)
    }

    pub fn receive_initial_response<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        encrypted: &[u8],
        generation: u64,
    ) -> Result<(BodyTimeTracker, BodyClockRateEstimator), BodyClockLineRefusal> {
        let (admission, exchange, correlation) =
            self.receive_exchange(membership, policy, session, timer, encrypted)?;
        let candidate = admission
            .derive(
                membership,
                policy,
                &self.peer_host,
                &self.peer_boot,
                &exchange,
                &correlation,
                generation,
            )
            .map_err(BodyClockLineRefusal::Admission)?;
        let tracker = BodyTimeTracker::new(candidate.clone(), &exchange.local_receive)
            .map_err(clock_refusal)?;
        let estimator = BodyClockRateEstimator::new(candidate).map_err(clock_refusal)?;
        Ok((tracker, estimator))
    }

    fn receive_exchange<T: TimerAdapter>(
        &mut self,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
        session: &mut ProtectedSession,
        timer: &mut T,
        encrypted: &[u8],
    ) -> Result<
        (
            BodyClockPeerAdmission,
            PeerClockExchange,
            BodyClockCorrelation,
        ),
        BodyClockLineRefusal,
    > {
        if self.role != Role::Initiator || self.pending.is_none() {
            return Err(BodyClockLineRefusal::Unexpected);
        }
        let admission = self
            .check(session, membership, policy)?
            .ok_or(BodyClockLineRefusal::Unexpected)?;
        let payload = open(session, encrypted)?;
        let local_receive = self.sample(timer)?;
        let response = decode(&payload)?;
        let (expected_sequence, local_send) = self
            .pending
            .take()
            .ok_or(BodyClockLineRefusal::Unexpected)?;
        self.last_sequence = expected_sequence;
        let ClockFrame::Response {
            version,
            body,
            sequence,
            receive,
            send,
            correlation,
        } = response
        else {
            return Err(BodyClockLineRefusal::Unexpected);
        };
        if version != WIRE_VERSION || body != self.body.as_str() || sequence != expected_sequence {
            return Err(BodyClockLineRefusal::Stale);
        }
        self.check(session, membership, policy)?;
        let exchange = PeerClockExchange {
            local_send,
            peer_receive: receive,
            peer_send: send,
            local_receive,
        };
        Ok((admission, exchange, *correlation))
    }

    fn sample<T: TimerAdapter>(
        &self,
        timer: &mut T,
    ) -> Result<MonotonicInstant, BodyClockLineRefusal> {
        let sample = timer
            .monotonic_observation(&self.local_host, &self.local_boot)
            .ok_or(BodyClockLineRefusal::NoClock)?;
        sample
            .validate()
            .map_err(|_| BodyClockLineRefusal::WrongClock)?;
        if sample.clock().host_id() != &self.local_host
            || sample.clock().boot_id() != &self.local_boot
        {
            return Err(BodyClockLineRefusal::WrongClock);
        }
        Ok(sample)
    }

    fn check(
        &self,
        session: &ProtectedSession,
        membership: &BodyMembership,
        policy: &BodyClockSourcePolicy,
    ) -> Result<Option<BodyClockPeerAdmission>, BodyClockLineRefusal> {
        let evidence = session.evidence();
        if evidence.disposition != SessionDisposition::Open {
            return Err(BodyClockLineRefusal::Closed);
        }
        if policy != &self.policy || membership.revision != self.membership_revision {
            return Err(BodyClockLineRefusal::Stale);
        }
        let (local, peer) = match self.role {
            Role::Initiator => (&self.binding.initiator, &self.binding.responder),
            Role::Responder => (&self.binding.responder, &self.binding.initiator),
        };
        if evidence.role != self.role
            || evidence.binding != &self.binding
            || self.binding.line_session_id.is_empty()
            || local.host_id != self.local_host.as_str()
            || local.boot_id != self.local_boot.as_str()
            || peer.host_id != self.peer_host.as_str()
            || peer.boot_id != self.peer_boot.as_str()
        {
            return Err(BodyClockLineRefusal::WrongSession);
        }
        if self.role == Role::Initiator {
            return BodyClockPeerAdmission::prepare(
                membership,
                &self.body,
                &self.part,
                &self.peer_host,
                &self.peer_boot,
                policy,
            )
            .map(Some)
            .map_err(BodyClockLineRefusal::Admission);
        }
        BodyClockPeerAdmission::prepare(
            membership,
            &self.body,
            &policy.part_id,
            &self.local_host,
            &self.local_boot,
            policy,
        )
        .map_err(BodyClockLineRefusal::Admission)?;
        let requester = membership
            .parts
            .iter()
            .find(|part| part.part_id == self.part)
            .ok_or(BodyClockLineRefusal::Stale)?;
        let requester_present = requester.current.as_ref().is_some_and(|observation| {
            observation.host_id == self.peer_host && observation.boot_id == self.peer_boot
        });
        if requester.state != MembershipState::Admitted || !requester_present {
            return Err(BodyClockLineRefusal::Stale);
        }
        Ok(None)
    }
}

fn seal(
    session: &mut ProtectedSession,
    frame: &ClockFrame,
    output: &mut [u8],
) -> Result<usize, BodyClockLineRefusal> {
    let payload = serde_json::to_vec(frame).map_err(|_| BodyClockLineRefusal::Malformed)?;
    if payload.len() > MAX_PAYLOAD {
        return Err(BodyClockLineRefusal::Malformed);
    }
    session
        .seal(&payload, output)
        .map_err(BodyClockLineRefusal::Transport)
}

fn open(session: &mut ProtectedSession, encrypted: &[u8]) -> Result<Vec<u8>, BodyClockLineRefusal> {
    if encrypted.len() > session.maximum_frame_bytes()
        || session.limits().maximum_payload_bytes as usize > MAX_PAYLOAD
    {
        return Err(BodyClockLineRefusal::Malformed);
    }
    let mut payload = [0; MAX_PAYLOAD];
    let length = session
        .open(encrypted, &mut payload)
        .map_err(BodyClockLineRefusal::Transport)?;
    Ok(payload[..length].to_vec())
}

fn decode(payload: &[u8]) -> Result<ClockFrame, BodyClockLineRefusal> {
    serde_json::from_slice(payload).map_err(|_| BodyClockLineRefusal::Malformed)
}

fn clock_refusal(error: BodyTimeRefusal) -> BodyClockLineRefusal {
    BodyClockLineRefusal::Admission(BodyClockAdmissionRefusal::Clock(error))
}

fn configure_tcp(
    stream: &TcpStream,
    policy: &BodyClockSourcePolicy,
) -> Result<Instant, BodyClockLineRefusal> {
    let ticks = policy.maximum_round_trip.ticks();
    let timeout = match policy.maximum_round_trip.scale() {
        conduit_core::TemporalScale::Seconds => Duration::from_secs(ticks),
        conduit_core::TemporalScale::Milliseconds => Duration::from_millis(ticks),
        conduit_core::TemporalScale::Microseconds => Duration::from_micros(ticks),
        conduit_core::TemporalScale::Nanoseconds => Duration::from_nanos(ticks),
    };
    if timeout.is_zero() {
        return Err(BodyClockLineRefusal::Stale);
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(BodyClockLineRefusal::Stale)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| BodyClockLineRefusal::Io(error.kind()))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| BodyClockLineRefusal::Io(error.kind()))?;
    Ok(deadline)
}

fn write_frame(
    stream: &mut TcpStream,
    frame: &[u8],
    deadline: Instant,
) -> Result<(), BodyClockLineRefusal> {
    let length = u32::try_from(frame.len()).map_err(|_| BodyClockLineRefusal::Malformed)?;
    if length == 0 {
        return Err(BodyClockLineRefusal::Malformed);
    }
    write_until(stream, &length.to_be_bytes(), deadline)?;
    write_until(stream, frame, deadline)
}

fn read_frame(
    stream: &mut TcpStream,
    maximum: usize,
    deadline: Instant,
) -> Result<Vec<u8>, BodyClockLineRefusal> {
    let mut header = [0; 4];
    read_until(stream, &mut header, deadline)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > maximum {
        return Err(BodyClockLineRefusal::Malformed);
    }
    let mut frame = vec![0; length];
    read_until(stream, &mut frame, deadline)?;
    Ok(frame)
}

fn read_until(
    stream: &mut TcpStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), BodyClockLineRefusal> {
    while !bytes.is_empty() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(BodyClockLineRefusal::Io(std::io::ErrorKind::TimedOut))?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|error| BodyClockLineRefusal::Io(error.kind()))?;
        match stream.read(bytes) {
            Ok(0) => return Err(BodyClockLineRefusal::Io(std::io::ErrorKind::UnexpectedEof)),
            Ok(length) => bytes = &mut bytes[length..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(BodyClockLineRefusal::Io(std::io::ErrorKind::TimedOut));
            }
            Err(error) => return Err(BodyClockLineRefusal::Io(error.kind())),
        }
    }
    Ok(())
}

fn write_until(
    stream: &mut TcpStream,
    mut bytes: &[u8],
    deadline: Instant,
) -> Result<(), BodyClockLineRefusal> {
    while !bytes.is_empty() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(BodyClockLineRefusal::Io(std::io::ErrorKind::TimedOut))?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(|error| BodyClockLineRefusal::Io(error.kind()))?;
        match stream.write(bytes) {
            Ok(0) => return Err(BodyClockLineRefusal::Io(std::io::ErrorKind::WriteZero)),
            Ok(length) => bytes = &bytes[length..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(BodyClockLineRefusal::Io(std::io::ErrorKind::TimedOut));
            }
            Err(error) => return Err(BodyClockLineRefusal::Io(error.kind())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{AuthenticatedHostObservation, Body, BodyMembership, MembershipProofId};
    use conduit_core::{
        ClockProvenance, MonotonicClockIdentity, MonotonicDuration, OfferGeneration, SignId,
        TemporalScale,
    };
    use conduit_protected_line::{
        EndpointBinding, ProtectedHandshake, ProtectedSessionPolicy, SessionLimits,
    };
    use std::time::Duration;

    struct NoClock;

    impl TimerAdapter for NoClock {
        fn wait(&mut self, _: Duration) {}
    }

    struct Samples(Vec<MonotonicInstant>);

    impl TimerAdapter for Samples {
        fn wait(&mut self, _: Duration) {}

        fn monotonic_observation(
            &mut self,
            _host_id: &HostId,
            _boot_id: &BootId,
        ) -> Option<MonotonicInstant> {
            if self.0.is_empty() {
                None
            } else {
                Some(self.0.remove(0))
            }
        }
    }

    fn sample(host: &str, boot: &str, ticks: u64) -> MonotonicInstant {
        MonotonicInstant::new(
            ticks,
            MonotonicClockIdentity::new(
                host.into(),
                boot.into(),
                "steady".into(),
                TemporalScale::Milliseconds,
                1,
                1,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn correlation(body: &BodyId, host: &str, boot: &str, anchor: u64) -> BodyClockCorrelation {
        BodyClockCorrelation::new(
            body.as_str().into(),
            TemporalScale::Milliseconds,
            1,
            sample(host, boot, anchor),
            10_000,
            0,
            100,
            2,
            100,
            ClockProvenance::External {
                provider_id: "provider/test".into(),
                admission_reference: "proof/test".into(),
                policy_id: "policy/test".into(),
            },
        )
        .unwrap()
    }

    fn session_binding() -> SessionBinding {
        SessionBinding {
            initiator: EndpointBinding {
                host_id: "host/local".into(),
                boot_id: "boot/local".into(),
            },
            responder: EndpointBinding {
                host_id: "host/peer".into(),
                boot_id: "boot/peer".into(),
            },
            negotiation_id: "negotiation/clock".into(),
            line_session_id: "line/clock".into(),
            candidate_binding: "candidate/clock".into(),
            transport_binding: "transport/clock".into(),
        }
    }

    fn session_policy() -> ProtectedSessionPolicy {
        ProtectedSessionPolicy {
            traffic: SessionLimits {
                maximum_payload_bytes: MAX_PAYLOAD as u32,
                maximum_frames_per_direction: 8,
                maximum_bytes_per_direction: 16_384,
            },
            maximum_simultaneous_sessions: 1,
            maximum_pending_frames_per_session: 1,
            handshake_work_units: 2,
            handshake_timeout_millis: 10_000,
            idle_timeout_millis: 10_000,
        }
    }

    fn sessions() -> (ProtectedSession, ProtectedSession) {
        let binding = session_binding();
        let limits = session_policy().traffic;
        let mut initiator =
            ProtectedHandshake::new(Role::Initiator, &binding, limits, [7; 32], [1; 32]).unwrap();
        let mut responder =
            ProtectedHandshake::new(Role::Responder, &binding, limits, [7; 32], [2; 32]).unwrap();
        let mut first = vec![0; initiator.next_message_bytes().unwrap()];
        initiator.write_message(&mut first).unwrap();
        responder.read_message(&first).unwrap();
        let mut second = vec![0; responder.next_message_bytes().unwrap()];
        responder.write_message(&mut second).unwrap();
        initiator.read_message(&second).unwrap();
        (initiator.finish().unwrap(), responder.finish().unwrap())
    }

    fn live_membership() -> (
        BodyId,
        BodyMembership,
        PartId,
        PartId,
        BodyClockSourcePolicy,
    ) {
        let body = Body::born(
            "source/process-clock".into(),
            "checked/process-clock".into(),
            1,
            SignId::from("sign/process-clock"),
        )
        .unwrap()
        .body_id;
        let local_part = PartId::bind(&body, "local", 1).unwrap();
        let peer_part = PartId::bind(&body, "peer", 2).unwrap();
        let mut membership = BodyMembership::new(body.clone()).unwrap();
        for (part, host, boot) in [
            (&local_part, "host/local", "boot/local"),
            (&peer_part, "host/peer", "boot/peer"),
        ] {
            let proof = MembershipProofId::bind(host).unwrap();
            membership
                .admit(
                    &body,
                    membership.revision,
                    part.clone(),
                    proof.clone(),
                    SignId::from(format!("sign/admit/{host}")),
                )
                .unwrap();
            membership
                .observe_present(
                    &body,
                    membership.revision,
                    part,
                    AuthenticatedHostObservation {
                        host_id: host.into(),
                        boot_id: boot.into(),
                        offer_generation: OfferGeneration(1),
                        proof_id: proof,
                        sequence: 1,
                    },
                    SignId::from(format!("sign/present/{host}")),
                )
                .unwrap();
        }
        let policy = BodyClockSourcePolicy {
            body_id: body.clone(),
            part_id: peer_part.clone(),
            membership_proof: MembershipProofId::bind("host/peer").unwrap(),
            policy_id: "policy/process-clock".into(),
            minimum_generation: 1,
            maximum_round_trip: MonotonicDuration::new(10_000_000, TemporalScale::Microseconds),
            maximum_local_rate_error_ppm: 1_000,
            correlation_horizon: MonotonicDuration::new(10_000_000, TemporalScale::Microseconds),
        };
        (body, membership, local_part, peer_part, policy)
    }

    #[test]
    #[ignore]
    fn real_process_clock_responder() {
        use std::net::TcpStream;

        let Ok(port) = std::env::var("CONDUIT_BODY_CLOCK_PEER_PORT") else {
            return;
        };
        let (body, membership, local_part, _, policy) = live_membership();
        let mut stream = TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap())).unwrap();
        let mut session = establish_body_clock_tcp_session(
            &mut stream,
            Role::Responder,
            &session_binding(),
            session_policy(),
            [7; 32],
        )
        .unwrap();
        let mut line = BodyClockLine::admit(
            &session,
            Role::Responder,
            body.clone(),
            "host/peer".into(),
            "boot/peer".into(),
            "host/local".into(),
            "boot/local".into(),
            local_part,
            &membership,
            &policy,
        )
        .unwrap();
        let mut timer = crate::ThreadTimer;
        let anchor = timer
            .monotonic_observation(&"host/peer".into(), &"boot/peer".into())
            .unwrap();
        let correlation = BodyClockCorrelation::new(
            body.as_str().into(),
            TemporalScale::Microseconds,
            1,
            anchor.clone(),
            1_000_000 + anchor.ticks(),
            0,
            1_000,
            1_000,
            10_000_000,
            ClockProvenance::External {
                provider_id: "provider/process-clock".into(),
                admission_reference: "proof/process-clock".into(),
                policy_id: "policy/process-clock".into(),
            },
        )
        .unwrap();
        line.respond_tcp(
            &membership,
            &policy,
            &mut session,
            &mut timer,
            &mut stream,
            &correlation,
        )
        .unwrap();
        println!("CLOCK_BASIS {}", anchor.clock().basis_id());
    }

    #[test]
    fn protected_tcp_bootstrap_uses_distinct_real_process_clocks() {
        use std::net::TcpListener;
        use std::process::{Command, Stdio};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "body_clock_line::tests::real_process_clock_responder",
                "--ignored",
                "--nocapture",
            ])
            .env(
                "CONDUIT_BODY_CLOCK_PEER_PORT",
                listener.local_addr().unwrap().port().to_string(),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                        child.kill().ok();
                        panic!("live clock peer did not connect");
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("live clock listener refused: {error}"),
            }
        };
        let (body, membership, _, peer_part, policy) = live_membership();
        let mut session = establish_body_clock_tcp_session(
            &mut stream,
            Role::Initiator,
            &session_binding(),
            session_policy(),
            [7; 32],
        )
        .unwrap();
        let mut line = BodyClockLine::admit(
            &session,
            Role::Initiator,
            body,
            "host/local".into(),
            "boot/local".into(),
            "host/peer".into(),
            "boot/peer".into(),
            peer_part,
            &membership,
            &policy,
        )
        .unwrap();
        let (tracker, estimator) = line
            .bootstrap_tcp(
                &membership,
                &policy,
                &mut session,
                &mut crate::ThreadTimer,
                &mut stream,
                2,
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output_text = String::from_utf8(output.stdout).unwrap();
        let peer_basis = output_text
            .split("CLOCK_BASIS ")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap();
        assert_ne!(tracker.last().local_sample.clock().basis_id(), peer_basis);
        assert_eq!(tracker.last().generation, 2);
        assert_eq!(estimator.latest(), tracker.correlation());
        assert!(matches!(
            &tracker.last().provenance,
            ClockProvenance::Peer { host_id, boot_id, .. }
                if host_id.as_str() == "host/peer" && boot_id.as_str() == "boot/peer"
        ));
    }

    #[test]
    fn tcp_clock_handshake_refuses_wrong_preshared_key() {
        use std::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut policy = session_policy();
        policy.handshake_timeout_millis = 200;
        let peer_policy = policy;
        let peer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            establish_body_clock_tcp_session(
                &mut stream,
                Role::Responder,
                &session_binding(),
                peer_policy,
                [8; 32],
            )
            .err()
        });
        let (mut stream, _) = listener.accept().unwrap();
        assert!(establish_body_clock_tcp_session(
            &mut stream,
            Role::Initiator,
            &session_binding(),
            policy,
            [7; 32],
        )
        .is_err());
        assert!(peer.join().unwrap().is_some());
    }

    #[test]
    fn protected_clock_exchange_crosses_bounded_tcp_frames() {
        use std::net::{TcpListener, TcpStream};

        let body = Body::born(
            "source/test".into(),
            "checked/test".into(),
            1,
            SignId::from("sign/born"),
        )
        .unwrap()
        .body_id;
        let local_part = PartId::bind(&body, "local", 1).unwrap();
        let peer_part = PartId::bind(&body, "peer", 2).unwrap();
        let mut membership = BodyMembership::new(body.clone()).unwrap();
        for (part, host, boot) in [
            (&local_part, "host/local", "boot/local"),
            (&peer_part, "host/peer", "boot/peer"),
        ] {
            let proof = MembershipProofId::bind(host).unwrap();
            membership
                .admit(
                    &body,
                    membership.revision,
                    part.clone(),
                    proof.clone(),
                    SignId::from(format!("sign/admit/{host}")),
                )
                .unwrap();
            membership
                .observe_present(
                    &body,
                    membership.revision,
                    part,
                    AuthenticatedHostObservation {
                        host_id: host.into(),
                        boot_id: boot.into(),
                        offer_generation: OfferGeneration(1),
                        proof_id: proof,
                        sequence: 1,
                    },
                    SignId::from(format!("sign/present/{host}")),
                )
                .unwrap();
        }
        let policy = BodyClockSourcePolicy {
            body_id: body.clone(),
            part_id: peer_part.clone(),
            membership_proof: MembershipProofId::bind("host/peer").unwrap(),
            policy_id: "policy/clock".into(),
            minimum_generation: 1,
            maximum_round_trip: MonotonicDuration::new(2_000, TemporalScale::Milliseconds),
            maximum_local_rate_error_ppm: 100,
            correlation_horizon: MonotonicDuration::new(2_000, TemporalScale::Milliseconds),
        };
        let (mut initiator_session, mut responder_session) = sessions();
        let mut initiator = BodyClockLine::admit(
            &initiator_session,
            Role::Initiator,
            body.clone(),
            "host/local".into(),
            "boot/local".into(),
            "host/peer".into(),
            "boot/peer".into(),
            peer_part,
            &membership,
            &policy,
        )
        .unwrap();
        let mut responder = BodyClockLine::admit(
            &responder_session,
            Role::Responder,
            body.clone(),
            "host/peer".into(),
            "boot/peer".into(),
            "host/local".into(),
            "boot/local".into(),
            local_part,
            &membership,
            &policy,
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer_membership = membership.clone();
        let peer_policy = policy.clone();
        let peer_body = body.clone();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            responder
                .respond_tcp(
                    &peer_membership,
                    &peer_policy,
                    &mut responder_session,
                    &mut Samples(vec![
                        sample("host/peer", "boot/peer", 8_005),
                        sample("host/peer", "boot/peer", 8_006),
                    ]),
                    &mut stream,
                    &correlation(&peer_body, "host/peer", "boot/peer", 8_000),
                )
                .unwrap();
        });
        let mut stream = TcpStream::connect(address).unwrap();
        let (tracker, estimator) = initiator
            .bootstrap_tcp(
                &membership,
                &policy,
                &mut initiator_session,
                &mut Samples(vec![
                    sample("host/local", "boot/local", 1_000),
                    sample("host/local", "boot/local", 1_010),
                ]),
                &mut stream,
                2,
            )
            .unwrap();
        peer.join().unwrap();
        assert_eq!(tracker.last().generation, 2);
        assert_eq!(tracker.last().local_sample.ticks(), 1_010);
        assert_eq!(estimator.latest(), tracker.correlation());
        assert!(matches!(
            &tracker.last().provenance,
            conduit_core::ClockProvenance::Peer { .. }
        ));
    }

    #[test]
    fn tcp_clock_framing_rejects_oversize_and_slow_peer() {
        use std::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(&2_049_u32.to_be_bytes()).unwrap();
        });
        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(
            read_frame(&mut stream, 2_048, Instant::now() + Duration::from_secs(1)),
            Err(BodyClockLineRefusal::Malformed)
        );
        writer.join().unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(&2_u32.to_be_bytes()).unwrap();
            stream.write_all(&[1]).unwrap();
            std::thread::sleep(Duration::from_millis(60));
        });
        let (mut stream, _) = listener.accept().unwrap();
        assert_eq!(
            read_frame(
                &mut stream,
                2_048,
                Instant::now() + Duration::from_millis(20)
            ),
            Err(BodyClockLineRefusal::Io(std::io::ErrorKind::TimedOut))
        );
        writer.join().unwrap();
    }

    #[test]
    fn protected_two_host_exchange_reconciles_only_admitted_source() {
        let body = Body::born(
            "source/test".into(),
            "checked/test".into(),
            1,
            SignId::from("sign/born"),
        )
        .unwrap()
        .body_id;
        let local_part = PartId::bind(&body, "local", 1).unwrap();
        let peer_part = PartId::bind(&body, "peer", 2).unwrap();
        let mut membership = BodyMembership::new(body.clone()).unwrap();
        for (part, host, boot) in [
            (&local_part, "host/local", "boot/local"),
            (&peer_part, "host/peer", "boot/peer"),
        ] {
            let proof = MembershipProofId::bind(host).unwrap();
            membership
                .admit(
                    &body,
                    membership.revision,
                    part.clone(),
                    proof.clone(),
                    SignId::from(format!("sign/admit/{host}")),
                )
                .unwrap();
            membership
                .observe_present(
                    &body,
                    membership.revision,
                    part,
                    AuthenticatedHostObservation {
                        host_id: host.into(),
                        boot_id: boot.into(),
                        offer_generation: OfferGeneration(1),
                        proof_id: proof,
                        sequence: 1,
                    },
                    SignId::from(format!("sign/present/{host}")),
                )
                .unwrap();
        }
        let policy = BodyClockSourcePolicy {
            body_id: body.clone(),
            part_id: peer_part.clone(),
            membership_proof: MembershipProofId::bind("host/peer").unwrap(),
            policy_id: "policy/clock".into(),
            minimum_generation: 1,
            maximum_round_trip: MonotonicDuration::new(20, TemporalScale::Milliseconds),
            maximum_local_rate_error_ppm: 100,
            correlation_horizon: MonotonicDuration::new(100, TemporalScale::Milliseconds),
        };
        let (mut initiator_session, mut responder_session) = sessions();
        let mut initiator = BodyClockLine::admit(
            &initiator_session,
            Role::Initiator,
            body.clone(),
            "host/local".into(),
            "boot/local".into(),
            "host/peer".into(),
            "boot/peer".into(),
            peer_part.clone(),
            &membership,
            &policy,
        )
        .unwrap();
        let mut responder = BodyClockLine::admit(
            &responder_session,
            Role::Responder,
            body.clone(),
            "host/peer".into(),
            "boot/peer".into(),
            "host/local".into(),
            "boot/local".into(),
            local_part,
            &membership,
            &policy,
        )
        .unwrap();
        let mut local_timer = Samples(vec![
            sample("host/local", "boot/local", 1_000),
            sample("host/local", "boot/local", 1_010),
        ]);
        let mut peer_timer = Samples(vec![
            sample("host/peer", "boot/peer", 8_005),
            sample("host/peer", "boot/peer", 8_006),
        ]);
        let mut request = vec![0; initiator_session.maximum_frame_bytes()];
        let request_length = initiator
            .send_request(
                &membership,
                &policy,
                &mut initiator_session,
                &mut local_timer,
                &mut request,
            )
            .unwrap();
        let mut response = vec![0; responder_session.maximum_frame_bytes()];
        let response_length = responder
            .respond(
                &membership,
                &policy,
                &mut responder_session,
                &mut peer_timer,
                &request[..request_length],
                &correlation(&body, "host/peer", "boot/peer", 8_000),
                &mut response,
            )
            .unwrap();
        let first = sample("host/local", "boot/local", 1_000);
        let mut tracker = BodyTimeTracker::new(
            correlation(&body, "host/local", "boot/local", 1_000),
            &first,
        )
        .unwrap();
        let mut estimator = BodyClockRateEstimator::new(tracker.correlation().clone()).unwrap();
        let estimate = initiator
            .receive_response(
                &membership,
                &policy,
                &mut initiator_session,
                &mut local_timer,
                &response[..response_length],
                2,
                &mut tracker,
                &mut estimator,
            )
            .unwrap();
        assert_eq!(estimate.generation, 2);
        assert_eq!(estimate.local_sample.ticks(), 1_010);
        assert_eq!(tracker.last(), &estimate);
        assert_eq!(estimator.retained_samples(), 2);
        let forged = ClockFrame::Request {
            version: WIRE_VERSION,
            body: body.as_str().into(),
            sequence: u64::MAX,
            send: sample("host/local", "boot/local", 1_020),
        };
        let forged_length = seal(&mut initiator_session, &forged, &mut request).unwrap();
        assert_eq!(
            responder.respond(
                &membership,
                &policy,
                &mut responder_session,
                &mut Samples(vec![sample("host/peer", "boot/peer", 8_020)]),
                &request[..forged_length],
                &correlation(&body, "host/peer", "boot/peer", 8_000),
                &mut response,
            ),
            Err(BodyClockLineRefusal::Stale)
        );
        assert_eq!(responder.last_sequence, 1);
        let mut interrupted_timer = Samples(vec![
            sample("host/local", "boot/local", 1_020),
            sample("host/local", "boot/local", 1_040),
            sample("host/local", "boot/local", 1_041),
            sample("host/local", "boot/local", 1_042),
            sample("host/local", "boot/local", 1_043),
            sample("host/local", "boot/local", 1_044),
            sample("host/local", "boot/local", 1_070),
        ]);
        let interrupted_length = initiator
            .send_request(
                &membership,
                &policy,
                &mut initiator_session,
                &mut interrupted_timer,
                &mut request,
            )
            .unwrap();
        let mut late_peer_timer = Samples(vec![
            sample("host/peer", "boot/peer", 8_025),
            sample("host/peer", "boot/peer", 8_026),
        ]);
        let late_length = responder
            .respond(
                &membership,
                &policy,
                &mut responder_session,
                &mut late_peer_timer,
                &request[..interrupted_length],
                &correlation(&body, "host/peer", "boot/peer", 8_000),
                &mut response,
            )
            .unwrap();
        assert!(!initiator.expire_pending(&mut interrupted_timer).unwrap());
        assert!(initiator.expire_pending(&mut interrupted_timer).unwrap());
        assert!(!initiator.expire_pending(&mut interrupted_timer).unwrap());
        initiator
            .send_request(
                &membership,
                &policy,
                &mut initiator_session,
                &mut interrupted_timer,
                &mut request,
            )
            .unwrap();
        assert_eq!(
            initiator.receive_response(
                &membership,
                &policy,
                &mut initiator_session,
                &mut interrupted_timer,
                &response[..late_length],
                3,
                &mut tracker,
                &mut estimator,
            ),
            Err(BodyClockLineRefusal::Stale)
        );
        assert_eq!(tracker.last(), &estimate);
        initiator
            .send_request(
                &membership,
                &policy,
                &mut initiator_session,
                &mut interrupted_timer,
                &mut request,
            )
            .unwrap();
        assert_eq!(initiator.last_sequence, 3);
        assert!(initiator.expire_pending(&mut interrupted_timer).unwrap());
        let mut revoked_policy = policy.clone();
        revoked_policy.policy_id = "policy/revoked".into();
        assert_eq!(
            initiator.send_request(
                &membership,
                &revoked_policy,
                &mut initiator_session,
                &mut local_timer,
                &mut request,
            ),
            Err(BodyClockLineRefusal::Stale)
        );
        membership
            .observe_offline(
                &body,
                membership.revision,
                &peer_part,
                &BootId::from("boot/peer"),
                SignId::from("sign/offline/peer"),
            )
            .unwrap();
        assert_eq!(
            initiator.send_request(
                &membership,
                &policy,
                &mut initiator_session,
                &mut local_timer,
                &mut request,
            ),
            Err(BodyClockLineRefusal::Stale)
        );
    }

    #[test]
    fn malformed_or_extended_frame_is_refused() {
        assert!(matches!(
            decode(b"not json"),
            Err(BodyClockLineRefusal::Malformed)
        ));
        assert!(matches!(
            decode(br#"{"Request":{"version":1,"body":"b","sequence":1,"send":null,"extra":1}}"#),
            Err(BodyClockLineRefusal::Malformed)
        ));
    }

    #[test]
    fn adapter_without_identified_monotonic_sample_is_refused() {
        let host = HostId::from("local");
        let boot = BootId::from("boot");
        let body = conduit_body::Body::born(
            "source/test".into(),
            "checked/test".into(),
            1,
            conduit_core::SignId::from("sign/born"),
        )
        .unwrap()
        .body_id;
        let part = PartId::bind(&body, "part", 1).unwrap();
        let line = BodyClockLine {
            binding: SessionBinding {
                initiator: conduit_protected_line::EndpointBinding {
                    host_id: "local".into(),
                    boot_id: "boot".into(),
                },
                responder: conduit_protected_line::EndpointBinding {
                    host_id: "peer".into(),
                    boot_id: "peer-boot".into(),
                },
                negotiation_id: "n".into(),
                line_session_id: "s".into(),
                candidate_binding: "c".into(),
                transport_binding: "t".into(),
            },
            role: Role::Initiator,
            body: body.clone(),
            local_host: host,
            local_boot: boot,
            peer_host: HostId::from("peer"),
            peer_boot: BootId::from("peer-boot"),
            part,
            policy: BodyClockSourcePolicy {
                body_id: body.clone(),
                part_id: PartId::bind(&body, "peer", 2).unwrap(),
                membership_proof: MembershipProofId::bind("peer").unwrap(),
                policy_id: "policy/test".into(),
                minimum_generation: 1,
                maximum_round_trip: MonotonicDuration::new(20, TemporalScale::Milliseconds),
                maximum_local_rate_error_ppm: 100,
                correlation_horizon: MonotonicDuration::new(100, TemporalScale::Milliseconds),
            },
            membership_revision: conduit_body::BodyMembershipRevision(0),
            pending: None,
            last_sequence: 0,
        };
        assert!(matches!(
            line.sample(&mut NoClock),
            Err(BodyClockLineRefusal::NoClock)
        ));
    }
}
