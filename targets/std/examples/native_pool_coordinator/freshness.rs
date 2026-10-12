//! Conservative freshness at a coordinator instant, using the existing
//! four-stamp correlation law. The caller must admit the producing clock
//! source and capture these stamps on its actual owned request/response pipe.
//! Serialized stamps/policy alone are not admission or authentication.
use conduit_core::*;
use conduit_planner::resource_cost::CostObservationWindow;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(remote = "PeerClockExchange")]
struct ExchangeWire {
    local_send: MonotonicInstant,
    peer_receive: MonotonicInstant,
    peer_send: MonotonicInstant,
    local_receive: MonotonicInstant,
}
#[derive(Deserialize)]
#[serde(remote = "PeerClockPolicy")]
struct PolicyWire {
    peer_host: HostId,
    peer_boot: BootId,
    body_basis: String,
    minimum_generation: u64,
    maximum_round_trip: MonotonicDuration,
    maximum_local_rate_error_ppm: u32,
    correlation_horizon: MonotonicDuration,
    admission_id: String,
    policy_id: String,
    membership_revision: u64,
    observation_sequence: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    #[serde(with = "ExchangeWire")]
    pub exchange: PeerClockExchange,
    #[serde(with = "PolicyWire")]
    pub policy: PeerClockPolicy,
    pub now: MonotonicInstant,
}

pub(super) fn validate(
    input: &Input,
    window: &CostObservationWindow,
    owner_validated_at: &MonotonicInstant,
) -> Result<BodyTimeEstimate, String> {
    let exchange = &input.exchange;
    let policy = &input.policy;
    if exchange.peer_receive != *window.observed()
        || exchange.peer_send != *owner_validated_at
        || policy.peer_host != *window.observed().clock().host_id()
        || policy.peer_boot != *window.observed().clock().boot_id()
        || input.now.clock() != exchange.local_receive.clock()
        || input.now.ticks() < exchange.local_receive.ticks()
        || policy.minimum_generation == 0
        || policy.membership_revision == 0
        || policy.observation_sequence == 0
        || policy.maximum_round_trip.scale() != TemporalScale::Microseconds
        || !(1..=500_000).contains(&policy.maximum_round_trip.ticks())
        || policy.correlation_horizon.scale() != TemporalScale::Microseconds
        || !(1..=1_000_000).contains(&policy.correlation_horizon.ticks())
        || policy.maximum_local_rate_error_ppm > 1000
    {
        return Err("sample exchange binding or admitted clock bounds changed".into());
    }
    // Define the reference coordinates to be this owner's own clock ticks.
    // This identity convention supplies no relationship to the coordinator;
    // that relationship is derived solely from the observed exchange below.
    let reference = BodyClockCorrelation::new(
        policy.body_basis.clone(),
        TemporalScale::Microseconds,
        policy.minimum_generation,
        exchange.peer_receive.clone(),
        exchange.peer_receive.ticks(),
        0,
        0,
        0,
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
    .map_err(|e| format!("owner clock reference: {e:?}"))?;
    let generation = policy
        .minimum_generation
        .checked_add(1)
        .ok_or("clock generation overflow")?;
    let correlation = exchange
        .derive_correlation(&reference, policy, generation)
        .map_err(|e| format!("sample clock exchange: {e:?}"))?;
    let estimate = correlation
        .project(&input.now)
        .map_err(|e| format!("sample current clock projection: {e:?}"))?;
    // Validate every possible current owner instant, not just the midpoint.
    // These are interval endpoints, never fabricated measured timestamps.
    for ticks in [estimate.earliest_ticks, estimate.latest_ticks] {
        let endpoint = MonotonicInstant::new(ticks, window.observed().clock().clone())
            .map_err(|e| format!("sample freshness endpoint: {e:?}"))?;
        window
            .validate_at(&endpoint)
            .map_err(|e| format!("sample current freshness: {e:?}"))?;
    }
    Ok(estimate)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Input, CostObservationWindow) {
        let clock = |host: &str| {
            MonotonicClockIdentity::new(
                HostId::from(host),
                BootId::from(format!("boot/{host}")),
                format!("clock/{host}"),
                TemporalScale::Microseconds,
                1,
                1,
            )
            .unwrap()
        };
        let local = clock("coordinator");
        let peer = clock("worker");
        let instant = |tick, clock: &MonotonicClockIdentity| {
            MonotonicInstant::new(tick, clock.clone()).unwrap()
        };
        let input = Input {
            exchange: PeerClockExchange {
                local_send: instant(100, &local),
                peer_receive: instant(10_000, &peer),
                peer_send: instant(10_020, &peer),
                local_receive: instant(150, &local),
            },
            policy: PeerClockPolicy {
                peer_host: HostId::from("worker"),
                peer_boot: BootId::from("boot/worker"),
                body_basis: "body/owner-reference".into(),
                minimum_generation: 1,
                maximum_round_trip: MonotonicDuration::new(500_000, TemporalScale::Microseconds),
                maximum_local_rate_error_ppm: 1000,
                correlation_horizon: MonotonicDuration::new(1_000_000, TemporalScale::Microseconds),
                admission_id: "fixture/owned-clock-source".into(),
                policy_id: "fixture/clock-policy".into(),
                membership_revision: 1,
                observation_sequence: 1,
            },
            now: instant(200, &local),
        };
        let window =
            CostObservationWindow::admit(instant(10_000, &peer), instant(11_000, &peer)).unwrap();
        (input, window)
    }
    #[test]
    fn different_clock_origins_are_related_by_actual_exchange_bounds() {
        let (input, window) = fixture();
        let estimate = validate(&input, &window, &input.exchange.peer_send).unwrap();
        assert!(estimate.earliest_ticks >= 10_020);
        assert!(estimate.latest_ticks < 11_000);
    }
    #[test]
    fn expired_interval_and_excessive_round_trip_refuse() {
        let (mut input, window) = fixture();
        input.now = MonotonicInstant::new(1200, input.now.clock().clone()).unwrap();
        assert!(validate(&input, &window, &input.exchange.peer_send).is_err());
        input.policy.maximum_round_trip = MonotonicDuration::new(10, TemporalScale::Microseconds);
        assert!(validate(&input, &window, &input.exchange.peer_send).is_err());
    }
    #[test]
    fn foreign_owner_and_current_clock_refuse() {
        let (mut input, window) = fixture();
        input.policy.peer_boot = BootId::from("boot/foreign");
        assert!(validate(&input, &window, &input.exchange.peer_send).is_err());
        let (mut input, window) = fixture();
        input.now = input.exchange.peer_send.clone();
        assert!(validate(&input, &window, &input.exchange.peer_send).is_err());
    }
}
