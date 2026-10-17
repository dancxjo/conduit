//! A conservative four-stamp peer exchange for Body clock establishment.

use alloc::string::String;

use crate::{
    BodyClockCorrelation, BodyTimeRefusal, BootId, ClockProvenance, HostId, MonotonicDuration,
    MonotonicInstant, TemporalScale, MAXIMUM_TEMPORAL_IDENTITY_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerClockPolicy {
    pub peer_host: HostId,
    pub peer_boot: BootId,
    pub body_basis: String,
    pub minimum_generation: u64,
    pub maximum_round_trip: MonotonicDuration,
    pub maximum_local_rate_error_ppm: u32,
    pub correlation_horizon: MonotonicDuration,
    pub admission_id: String,
    pub policy_id: String,
    pub membership_revision: u64,
    pub observation_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerClockExchange {
    pub local_send: MonotonicInstant,
    pub peer_receive: MonotonicInstant,
    pub peer_send: MonotonicInstant,
    pub local_receive: MonotonicInstant,
}

impl PeerClockExchange {
    pub fn derive_correlation(
        &self,
        peer_correlation: &BodyClockCorrelation,
        policy: &PeerClockPolicy,
        new_generation: u64,
    ) -> Result<BodyClockCorrelation, BodyTimeRefusal> {
        if policy.admission_id.is_empty()
            || policy.admission_id.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || policy.policy_id.is_empty()
            || policy.policy_id.len() > MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || policy.maximum_local_rate_error_ppm >= 1_000_000
            || policy.correlation_horizon.ticks() == 0
            || policy.maximum_round_trip.scale() != self.local_send.clock().scale()
            || policy.correlation_horizon.scale() != self.local_send.clock().scale()
        {
            return Err(BodyTimeRefusal::InvalidExchange);
        }
        if policy.peer_host != *self.peer_receive.clock().host_id()
            || policy.peer_boot != *self.peer_receive.clock().boot_id()
            || policy.body_basis != peer_correlation.body_basis()
            || peer_correlation.generation() < policy.minimum_generation
            || new_generation <= peer_correlation.generation()
        {
            return Err(BodyTimeRefusal::UnadmittedPeer);
        }
        if self.local_send.clock() != self.local_receive.clock()
            || self.peer_receive.clock() != self.peer_send.clock()
            || self.peer_receive.clock() != peer_correlation.local_clock()
            || self.local_send.clock().host_id() == self.peer_receive.clock().host_id()
            || self.local_receive.ticks() < self.local_send.ticks()
            || self.peer_send.ticks() < self.peer_receive.ticks()
        {
            return Err(BodyTimeRefusal::InvalidExchange);
        }
        let round_trip = self.local_receive.ticks() - self.local_send.ticks();
        if round_trip > policy.maximum_round_trip.ticks() {
            return Err(BodyTimeRefusal::ExcessiveRoundTrip);
        }
        peer_correlation.project(&self.peer_receive)?;
        let peer_send = peer_correlation.project(&self.peer_send)?;
        let local_quantum = u128::from(nanoseconds_per_tick(self.local_send.clock().scale()));
        let body_quantum = u128::from(nanoseconds_per_tick(peer_correlation.body_scale()));
        let endpoint_error = u128::from(self.local_send.clock().uncertainty_ticks())
            + u128::from(self.local_receive.clock().uncertainty_ticks())
            + u128::from(self.local_send.clock().resolution_ticks())
            + u128::from(self.local_receive.clock().resolution_ticks());
        let maximum_flight = ((u128::from(round_trip) + endpoint_error)
            * local_quantum
            * 1_000_000)
            .div_ceil(body_quantum * (1_000_000 - u128::from(policy.maximum_local_rate_error_ppm)));
        let upper = u128::from(peer_send.latest_ticks)
            .checked_add(maximum_flight)
            .ok_or(BodyTimeRefusal::Overflow)?;
        let lower = u128::from(peer_send.earliest_ticks);
        let center = (lower + upper) / 2;
        let uncertainty = (upper - lower).div_ceil(2);
        BodyClockCorrelation::new(
            policy.body_basis.clone(),
            peer_correlation.body_scale(),
            new_generation,
            self.local_receive.clone(),
            u64::try_from(center).map_err(|_| BodyTimeRefusal::Overflow)?,
            0,
            policy.maximum_local_rate_error_ppm,
            u64::try_from(uncertainty).map_err(|_| BodyTimeRefusal::Overflow)?,
            policy.correlation_horizon.ticks(),
            ClockProvenance::Peer {
                host_id: policy.peer_host.clone(),
                boot_id: policy.peer_boot.clone(),
                admission_reference: policy.admission_id.clone(),
                policy_id: policy.policy_id.clone(),
                membership_revision: policy.membership_revision,
                observation_sequence: policy.observation_sequence,
            },
        )
    }
}

const fn nanoseconds_per_tick(scale: TemporalScale) -> u64 {
    match scale {
        TemporalScale::Seconds => 1_000_000_000,
        TemporalScale::Milliseconds => 1_000_000,
        TemporalScale::Microseconds => 1_000,
        TemporalScale::Nanoseconds => 1,
    }
}
