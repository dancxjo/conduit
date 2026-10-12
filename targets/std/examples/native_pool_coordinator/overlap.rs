//! A shared coordinator instant must lie strictly inside both actual native
//! adapter-call intervals under every admitted clock projection. Owned pipe
//! provenance is supplied by the proof owner; serialized reports grant none.
use conduit_core::*;
use serde::Deserialize;
use serde_json::json;
use std::io::{Read, Write};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    phase: String,
    schema: String,
    scope: String,
    selection: PoolSelectionEvidence,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    capability_id: CapabilityId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    started: MonotonicInstant,
    completed: MonotonicInstant,
    elapsed: MonotonicDuration,
    input_bytes: u32,
    output_bytes: u32,
    input_digest: [u8; 32],
    terminal: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    plan: Plan,
    completions: [Completion; 2],
    clocks: [super::freshness::Input; 2],
}
fn project(input: &super::freshness::Input) -> Result<BodyTimeEstimate, String> {
    let exchange = &input.exchange;
    let policy = &input.policy;
    if policy.minimum_generation == 0
        || policy.membership_revision == 0
        || policy.observation_sequence == 0
        || policy.maximum_round_trip.scale() != TemporalScale::Microseconds
        || !(1..=500_000).contains(&policy.maximum_round_trip.ticks())
        || policy.correlation_horizon.scale() != TemporalScale::Microseconds
        || !(1..=30_000_000).contains(&policy.correlation_horizon.ticks())
        || policy.maximum_local_rate_error_ppm > 1000
        || exchange.peer_receive.clock().scale() != TemporalScale::Microseconds
    {
        return Err("overlap clock bounds refused".into());
    }
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
    .map_err(|e| format!("overlap owner clock: {e:?}"))?;
    let generation = policy
        .minimum_generation
        .checked_add(1)
        .ok_or("clock generation overflow")?;
    exchange
        .derive_correlation(&reference, policy, generation)
        .and_then(|correlation| correlation.project(&input.now))
        .map_err(|e| format!("overlap clock projection: {e:?}"))
}
fn ensure_inside(
    started: &MonotonicInstant,
    completed: &MonotonicInstant,
    estimate: &BodyTimeEstimate,
) -> Result<(), String> {
    let endpoint_error = started
        .clock()
        .uncertainty_ticks()
        .checked_add(started.clock().resolution_ticks())
        .ok_or("completion clock error overflow")?;
    let start_latest = started
        .ticks()
        .checked_add(endpoint_error)
        .ok_or("completion start overflow")?;
    let end_earliest = completed
        .ticks()
        .checked_sub(endpoint_error)
        .ok_or("completion end underflow")?;
    if start_latest >= estimate.earliest_ticks || estimate.latest_ticks >= end_earliest {
        return Err(format!("bounded common instant does not prove native call overlap: call {start_latest}..{end_earliest}, witness {}..{}", estimate.earliest_ticks, estimate.latest_ticks));
    }
    Ok(())
}
fn check(input: &Input) -> Result<serde_json::Value, String> {
    let [first, second] = &input.completions;
    if first.host_id == second.host_id
        || first.selection.operation_id == second.selection.operation_id
        || input.clocks[0].now != input.clocks[1].now
    {
        return Err("overlap requires distinct Hosts/operations and one common instant".into());
    }
    let mut projections = Vec::with_capacity(2);
    for (receipt, clock) in input.completions.iter().zip(&input.clocks) {
        receipt
            .selection
            .validate(&input.plan)
            .map_err(|e| format!("completion selection: {e:?}"))?;
        let pool = input
            .plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == receipt.selection.pool_id)
            })
            .ok_or("completion pool absent")?;
        let index = receipt
            .selection
            .selected_realization
            .ok_or("completion lacks selected realization")?;
        let realization = pool
            .realization_envelope
            .get(usize::from(index))
            .ok_or("completion realization absent")?;
        if receipt.phase != "measured"
            || receipt.schema != "conduit.proof/native-completed-work@1"
            || receipt.scope != "adapter_call_only"
            || !matches!(receipt.terminal.as_str(), "Produced" | "Truncated")
            || receipt.host_id != realization.host_id
            || receipt.boot_id != realization.boot_id
            || receipt.offer_generation != realization.offer_generation
            || receipt.capability_id != realization.capability_id
            || receipt.implementation_id != realization.implementation_id
            || receipt.artifact_id != realization.artifact_id
            || receipt.started.clock() != clock.exchange.peer_receive.clock()
            || receipt.completed.clock() != receipt.started.clock()
            || receipt.started.clock().host_id() != &receipt.host_id
            || receipt.started.clock().boot_id() != &receipt.boot_id
        {
            return Err("completion identity, measurement scope or clock changed".into());
        }
        if receipt
            .completed
            .elapsed_since(&receipt.started)
            .map_err(|e| format!("completion interval: {e:?}"))?
            != receipt.elapsed
        {
            return Err("completion elapsed duration changed".into());
        }
        let estimate = project(clock)?;
        ensure_inside(&receipt.started, &receipt.completed, &estimate)?;
        projections.push(json!({"host_id":receipt.host_id,"operation":receipt.selection.operation_id,
            "projected_witness":estimate,"started":receipt.started,"completed":receipt.completed,
            "input_bytes":receipt.input_bytes,"output_bytes":receipt.output_bytes,"input_digest":receipt.input_digest}));
    }
    Ok(
        json!({"phase":"native-call-overlap-proven","scope":"adapter_call_only","common_instant":input.clocks[0].now,"projections":projections}),
    )
}
pub(super) fn run() -> Result<(), String> {
    let mut input = std::io::stdin().lock();
    let mut length = [0; 4];
    input
        .read_exact(&mut length)
        .map_err(|_| "overlap header unavailable")?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > 1024 * 1024 {
        return Err("overlap document extent refused".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|_| "overlap document incomplete")?;
    let request = serde_json::from_slice(&bytes).map_err(|_| "overlap document invalid")?;
    let result = serde_json::to_vec(&check(&request)?).map_err(|_| "overlap receipt invalid")?;
    if result.len() > 1024 * 1024 {
        return Err("overlap receipt extent refused".into());
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(result.len() as u32).to_be_bytes())
        .and_then(|_| output.write_all(&result))
        .map_err(|_| "overlap receipt unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (super::super::freshness::Input, MonotonicClockIdentity) {
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
        let local = clock("controller");
        let peer = clock("worker");
        let instant = |ticks, clock: &MonotonicClockIdentity| {
            MonotonicInstant::new(ticks, clock.clone()).unwrap()
        };
        (
            super::super::freshness::Input {
                exchange: PeerClockExchange {
                    local_send: instant(100, &local),
                    local_receive: instant(150, &local),
                    peer_receive: instant(10_000, &peer),
                    peer_send: instant(10_020, &peer),
                },
                policy: PeerClockPolicy {
                    peer_host: HostId::from("worker"),
                    peer_boot: BootId::from("boot/worker"),
                    body_basis: "reference/worker".into(),
                    minimum_generation: 1,
                    maximum_round_trip: MonotonicDuration::new(
                        500_000,
                        TemporalScale::Microseconds,
                    ),
                    maximum_local_rate_error_ppm: 1000,
                    correlation_horizon: MonotonicDuration::new(
                        30_000_000,
                        TemporalScale::Microseconds,
                    ),
                    admission_id: "fixture/owned-clock".into(),
                    policy_id: "fixture/clock-policy".into(),
                    membership_revision: 1,
                    observation_sequence: 1,
                },
                now: instant(200, &local),
            },
            peer,
        )
    }
    #[test]
    fn midpoint_inside_is_insufficient_when_uncertainty_crosses_start() {
        let (input, clock) = fixture();
        let estimate = project(&input).unwrap();
        let started = MonotonicInstant::new(estimate.earliest_ticks, clock.clone()).unwrap();
        let completed = MonotonicInstant::new(estimate.latest_ticks + 10, clock).unwrap();
        assert!(
            started.ticks() < estimate.center_ticks && estimate.center_ticks < completed.ticks()
        );
        assert!(ensure_inside(&started, &completed, &estimate).is_err());
    }
    #[test]
    fn endpoint_uncertainty_is_retained_at_both_native_call_edges() {
        let (input, clock) = fixture();
        let estimate = project(&input).unwrap();
        let instant = |ticks| MonotonicInstant::new(ticks, clock.clone()).unwrap();
        assert!(ensure_inside(
            &instant(estimate.earliest_ticks - 2),
            &instant(estimate.latest_ticks + 10),
            &estimate
        )
        .is_err());
        assert!(ensure_inside(
            &instant(estimate.earliest_ticks - 10),
            &instant(estimate.latest_ticks + 2),
            &estimate
        )
        .is_err());
        assert!(ensure_inside(
            &instant(estimate.earliest_ticks - 10),
            &instant(estimate.latest_ticks + 10),
            &estimate
        )
        .is_ok());
    }
    #[test]
    fn expired_correlation_and_excessive_round_trip_refuse() {
        let (mut input, _) = fixture();
        input.now = MonotonicInstant::new(30_000_151, input.now.clock().clone()).unwrap();
        assert!(project(&input).is_err());
        let (mut input, _) = fixture();
        input.policy.maximum_round_trip = MonotonicDuration::new(10, TemporalScale::Microseconds);
        assert!(project(&input).is_err());
    }
}
