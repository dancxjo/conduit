//! Bounded authenticated Body clock exchange over an already established line.

use conduit_body::{
    BodyClockAdmissionRefusal, BodyClockPeerAdmission, BodyClockSourcePolicy, BodyId,
    BodyMembership, MembershipState, PartId,
};
use conduit_core::{
    BodyClockCorrelation, BodyClockRateEstimator, BodyTimeEstimate, BodyTimeTracker, BootId,
    HostId, MonotonicInstant, PeerClockExchange,
};
use conduit_protected_line::{
    ProtectedLineError, ProtectedSession, Role, SessionBinding, SessionDisposition,
};
use serde::{Deserialize, Serialize};

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
        correlation: BodyClockCorrelation,
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
            correlation: local_correlation.clone(),
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
        if version != WIRE_VERSION
            || body != self.body.as_str()
            || sequence != expected_sequence
            || sequence <= self.last_sequence
        {
            return Err(BodyClockLineRefusal::Stale);
        }
        self.check(session, membership, policy)?;
        let exchange = PeerClockExchange {
            local_send,
            peer_receive: receive,
            peer_send: send,
            local_receive,
        };
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
        self.last_sequence = sequence;
        Ok(estimate)
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{AuthenticatedHostObservation, Body, BodyMembership, MembershipProofId};
    use conduit_core::{
        ClockProvenance, MonotonicClockIdentity, MonotonicDuration, OfferGeneration, SignId,
        TemporalScale,
    };
    use conduit_protected_line::{EndpointBinding, ProtectedHandshake, SessionLimits};
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

    fn sessions() -> (ProtectedSession, ProtectedSession) {
        let binding = SessionBinding {
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
        };
        let limits = SessionLimits {
            maximum_payload_bytes: MAX_PAYLOAD as u32,
            maximum_frames_per_direction: 8,
            maximum_bytes_per_direction: 16_384,
        };
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
