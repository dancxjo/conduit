//! Manual measurements of the actual atomic selector, never provider latency.
use conduit_kernel::{shared_pool::*, NodeId};
use std::{hint::black_box, time::Instant};

struct Fixture {
    envelope: Vec<LoweredPoolRealization>,
    requirements: Vec<LoweredPoolResourceRequirement>,
    observations: Vec<LoweredPoolObservation>,
    costs: Vec<LoweredPoolCost>,
}
impl Fixture {
    fn new(count: u16, scenario: &str) -> Self {
        let envelope: Vec<_> = (0..count)
            .map(|i| LoweredPoolRealization {
                realization: i,
                placement: MemberPlacement {
                    node: NodeId(i),
                    realization: i,
                    play: 7,
                },
                boot: i + 10,
                offer_generation: 2,
                capability: i,
                implementation: i,
                artifact: i,
                member_capacity: 1,
            })
            .collect();
        let requirements = envelope
            .iter()
            .map(|r| LoweredPoolResourceRequirement {
                realization: r.realization,
                resource: 0,
                units: 1,
            })
            .collect();
        let observations = envelope
            .iter()
            .map(|r| LoweredPoolObservation {
                realization: r.realization,
                resource: 0,
                boot: r.boot,
                offer_generation: r.offer_generation,
                capability: r.capability,
                implementation: r.implementation,
                artifact: r.artifact,
                health: if scenario == "fastest-unavailable"
                    && count > 1
                    && r.realization == count - 1
                {
                    LoweredObservationHealth::Unavailable
                } else {
                    LoweredObservationHealth::Ready
                },
                unreserved_units: 100 - u32::from(r.realization),
                utilized_units: 0,
                sign: r.realization + 40,
            })
            .collect();
        let costs = if scenario == "missing" {
            vec![]
        } else {
            envelope
                .iter()
                .map(|r| LoweredPoolCost {
                    realization: *r,
                    demand: [9; 32],
                    upper_completion: LoweredPoolDuration::from_nanoseconds(
                        u64::from(count - r.realization) * 100,
                    ),
                    source_clock: r.boot,
                    evaluation_clock: r.boot,
                    observed_ticks: 100,
                    expires_ticks: 110,
                    evaluation_ticks: if scenario == "stale" { 110 } else { 105 },
                    sign: r.realization + 80,
                })
                .collect()
        };
        Self {
            envelope,
            requirements,
            observations,
            costs,
        }
    }
    fn expected(&self, cost_aware: bool, scenario: &str) -> u16 {
        if !cost_aware || matches!(scenario, "missing" | "stale") {
            0
        } else if scenario == "fastest-unavailable" && self.envelope.len() > 1 {
            self.envelope.len() as u16 - 2
        } else {
            self.envelope.len() as u16 - 1
        }
    }
    fn batch(&self, cost_aware: bool, iterations: u32) -> (u128, u64) {
        let mut checksum = 0_u64;
        let mut evidence = [0_u16; 1];
        let mut decisions = [PoolCostCandidateDecision::EMPTY; 32];
        let start = Instant::now();
        for iteration in 0..iterations {
            // Both paths include identical fixed-pool construction and complete
            // lifecycle. Signs remain finite; no hidden clearing or replay.
            let mut pool =
                FixedSharedPool::<32, 8>::new(PoolId(1), 32, 7, self.envelope.len() as u16)
                    .unwrap();
            let mut bytes = [0_u8; 32];
            bytes[..4].copy_from_slice(&iteration.to_le_bytes());
            let key = MemberKey(bytes);
            let selected = if cost_aware {
                admit_cost_selected_pool_member(
                    &mut pool,
                    key,
                    7,
                    black_box(&self.envelope),
                    black_box(&self.requirements),
                    black_box(&self.observations),
                    PoolCostSelection {
                        demand: [9; 32],
                        costs: black_box(&self.costs),
                    },
                    &mut evidence,
                    &mut decisions,
                )
            } else {
                admit_selected_pool_member(
                    &mut pool,
                    key,
                    7,
                    black_box(&self.envelope),
                    black_box(&self.requirements),
                    black_box(&self.observations),
                    PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
                    &mut evidence,
                )
            }
            .unwrap();
            checksum += u64::from(black_box(selected.member.placement.realization)) + 1;
            pool.trigger(selected.member).unwrap();
            pool.request_release(selected.member).unwrap();
            pool.complete_release(selected.member).unwrap();
            assert_eq!(black_box(pool.population()), 0);
            assert_eq!(pool.signs().count(), 4);
        }
        (start.elapsed().as_nanos(), checksum)
    }
}

#[test]
#[ignore = "explicit characterization; fixture scheduling overhead, not physical provider performance"]
fn characterize_atomic_cost_selection_against_capacity_reference() {
    let iterations = 1_000_u32;
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    for count in [1, 8, 32] {
        for scenario in ["known", "missing", "stale", "fastest-unavailable"] {
            let fixture = Fixture::new(count, scenario);
            for trial in 0..8 {
                let mut reference = (0, 0);
                let mut cost = (0, 0);
                for cost_aware in if trial % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let result = fixture.batch(cost_aware, iterations);
                    assert_eq!(
                        result.1,
                        (u64::from(fixture.expected(cost_aware, scenario)) + 1)
                            * u64::from(iterations)
                    );
                    if cost_aware {
                        cost = result;
                    } else {
                        reference = result;
                    }
                }
                println!("{{\"scope\":\"fixed-pool-init-atomic-selection-trigger-release\",\"realizations\":{count},\"scenario\":\"{scenario}\",\"trial\":{trial},\"iterations\":{iterations},\"reference_ns\":{},\"cost_ns\":{},\"reference_checksum\":{},\"cost_checksum\":{},\"profile\":\"{profile}\",\"clock\":\"std::time::Instant\"}}", reference.0, cost.0, reference.1, cost.1);
            }
        }
    }
}
