use alloc::string::{String, ToString};
use conduit_core::{
    BodyClockCorrelation, BodyClockRateEstimator, BodyTimeEstimate, BodyTimeRefusal,
    BodyTimeTracker, BootId, HostId, MonotonicDuration, PeerClockExchange, PeerClockPolicy,
    MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

use crate::{
    AuthenticatedHostObservation, BodyId, BodyMembership, BodyMembershipRevision,
    MembershipProofId, MembershipRefusal, MembershipState, PartId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyClockAdmissionRefusal {
    Membership(MembershipRefusal),
    WrongBody,
    UnknownPart,
    UnavailablePeer,
    WrongTransportPeer,
    UnauthorizedClockSource,
    StaleMembership,
    StalePolicy,
    InvalidPolicy,
    Clock(BodyTimeRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyClockSourcePolicy {
    pub body_id: BodyId,
    pub part_id: PartId,
    pub membership_proof: MembershipProofId,
    pub policy_id: String,
    pub minimum_generation: u64,
    pub maximum_round_trip: MonotonicDuration,
    pub maximum_local_rate_error_ppm: u32,
    pub correlation_horizon: MonotonicDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyClockPeerAdmission {
    body_id: BodyId,
    part_id: PartId,
    membership_revision: BodyMembershipRevision,
    observation: AuthenticatedHostObservation,
    source_policy: BodyClockSourcePolicy,
    policy_id: String,
    minimum_generation: u64,
    maximum_round_trip: MonotonicDuration,
    maximum_local_rate_error_ppm: u32,
    correlation_horizon: MonotonicDuration,
}

impl BodyClockPeerAdmission {
    pub fn prepare(
        membership: &BodyMembership,
        body_id: &BodyId,
        part_id: &PartId,
        transport_host: &HostId,
        transport_boot: &BootId,
        source_policy: &BodyClockSourcePolicy,
    ) -> Result<Self, BodyClockAdmissionRefusal> {
        membership
            .validate()
            .map_err(BodyClockAdmissionRefusal::Membership)?;
        if &membership.body_id != body_id {
            return Err(BodyClockAdmissionRefusal::WrongBody);
        }
        if source_policy.policy_id.is_empty()
            || source_policy.policy_id.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || source_policy.maximum_local_rate_error_ppm >= 1_000_000
            || source_policy.maximum_round_trip.ticks() == 0
            || source_policy.correlation_horizon.ticks() == 0
            || source_policy.maximum_round_trip.scale() != source_policy.correlation_horizon.scale()
        {
            return Err(BodyClockAdmissionRefusal::InvalidPolicy);
        }
        let part = membership
            .parts
            .iter()
            .find(|part| &part.part_id == part_id)
            .ok_or(BodyClockAdmissionRefusal::UnknownPart)?;
        if source_policy.body_id != *body_id || source_policy.part_id != *part_id {
            return Err(BodyClockAdmissionRefusal::UnauthorizedClockSource);
        }
        if part.state != MembershipState::Admitted {
            return Err(BodyClockAdmissionRefusal::UnavailablePeer);
        }
        let observation = part
            .current
            .as_ref()
            .ok_or(BodyClockAdmissionRefusal::UnavailablePeer)?;
        if observation.proof_id != source_policy.membership_proof {
            return Err(BodyClockAdmissionRefusal::UnauthorizedClockSource);
        }
        if &observation.host_id != transport_host || &observation.boot_id != transport_boot {
            return Err(BodyClockAdmissionRefusal::WrongTransportPeer);
        }
        Ok(Self {
            body_id: body_id.clone(),
            part_id: part_id.clone(),
            membership_revision: membership.revision,
            observation: observation.clone(),
            source_policy: source_policy.clone(),
            policy_id: source_policy.policy_id.clone(),
            minimum_generation: source_policy.minimum_generation,
            maximum_round_trip: source_policy.maximum_round_trip,
            maximum_local_rate_error_ppm: source_policy.maximum_local_rate_error_ppm,
            correlation_horizon: source_policy.correlation_horizon,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn derive(
        &self,
        current_membership: &BodyMembership,
        current_policy: &BodyClockSourcePolicy,
        transport_host: &HostId,
        transport_boot: &BootId,
        exchange: &PeerClockExchange,
        peer_correlation: &BodyClockCorrelation,
        new_generation: u64,
    ) -> Result<BodyClockCorrelation, BodyClockAdmissionRefusal> {
        current_membership
            .validate()
            .map_err(BodyClockAdmissionRefusal::Membership)?;
        if current_membership.body_id != self.body_id {
            return Err(BodyClockAdmissionRefusal::WrongBody);
        }
        if current_policy != &self.source_policy {
            return Err(BodyClockAdmissionRefusal::StalePolicy);
        }
        if current_membership.revision != self.membership_revision {
            return Err(BodyClockAdmissionRefusal::StaleMembership);
        }
        let part = current_membership
            .parts
            .iter()
            .find(|part| part.part_id == self.part_id)
            .ok_or(BodyClockAdmissionRefusal::UnknownPart)?;
        if part.state != MembershipState::Admitted
            || part.current.as_ref() != Some(&self.observation)
        {
            return Err(BodyClockAdmissionRefusal::StaleMembership);
        }
        if transport_host != &self.observation.host_id
            || transport_boot != &self.observation.boot_id
        {
            return Err(BodyClockAdmissionRefusal::WrongTransportPeer);
        }
        if peer_correlation.body_basis() != self.body_id.as_str()
            || peer_correlation.local_clock().host_id() != transport_host
            || peer_correlation.local_clock().boot_id() != transport_boot
        {
            return Err(BodyClockAdmissionRefusal::WrongTransportPeer);
        }
        let policy = PeerClockPolicy {
            peer_host: self.observation.host_id.clone(),
            peer_boot: self.observation.boot_id.clone(),
            body_basis: self.body_id.as_str().to_string(),
            minimum_generation: self.minimum_generation,
            maximum_round_trip: self.maximum_round_trip,
            maximum_local_rate_error_ppm: self.maximum_local_rate_error_ppm,
            correlation_horizon: self.correlation_horizon,
            admission_id: self.observation.proof_id.as_str().to_string(),
            policy_id: self.policy_id.clone(),
            membership_revision: self.membership_revision.0,
            observation_sequence: self.observation.sequence,
        };
        exchange
            .derive_correlation(peer_correlation, &policy, new_generation)
            .map_err(BodyClockAdmissionRefusal::Clock)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconcile<'a>(
        &self,
        current_membership: &BodyMembership,
        current_policy: &BodyClockSourcePolicy,
        transport_host: &HostId,
        transport_boot: &BootId,
        exchange: &PeerClockExchange,
        peer_correlation: &BodyClockCorrelation,
        new_generation: u64,
        tracker: &'a mut BodyTimeTracker,
    ) -> Result<&'a BodyTimeEstimate, BodyClockAdmissionRefusal> {
        let correlation = self.derive(
            current_membership,
            current_policy,
            transport_host,
            transport_boot,
            exchange,
            peer_correlation,
            new_generation,
        )?;
        tracker
            .reconcile(correlation, &exchange.local_receive)
            .map_err(BodyClockAdmissionRefusal::Clock)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reconcile_with_rate_estimator<'a>(
        &self,
        current_membership: &BodyMembership,
        current_policy: &BodyClockSourcePolicy,
        transport_host: &HostId,
        transport_boot: &BootId,
        exchange: &PeerClockExchange,
        peer_correlation: &BodyClockCorrelation,
        new_generation: u64,
        tracker: &'a mut BodyTimeTracker,
        estimator: &mut BodyClockRateEstimator,
    ) -> Result<&'a BodyTimeEstimate, BodyClockAdmissionRefusal> {
        if estimator.latest() != tracker.correlation() {
            return Err(BodyClockAdmissionRefusal::Clock(
                BodyTimeRefusal::ConflictingEvidence,
            ));
        }
        let candidate = self.derive(
            current_membership,
            current_policy,
            transport_host,
            transport_boot,
            exchange,
            peer_correlation,
            new_generation,
        )?;
        let refined = estimator
            .refine(&candidate, self.maximum_local_rate_error_ppm)
            .map_err(BodyClockAdmissionRefusal::Clock)?;
        let estimate = tracker
            .reconcile(refined.clone(), &exchange.local_receive)
            .map_err(BodyClockAdmissionRefusal::Clock)?;
        estimator
            .record(refined)
            .map_err(BodyClockAdmissionRefusal::Clock)?;
        Ok(estimate)
    }
}
