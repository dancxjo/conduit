//! Time the production selector, including validation, exact observation Sign
//! capture and atomic occupation. It is not a benchmark-only scheduler.
use crate::{
    config::Config,
    timing::{self, Metrics},
};
use conduit_kernel::{shared_pool::*, NodeId};
use std::{hint::black_box, time::Instant};
pub struct Fixture {
    pub envelope: Vec<LoweredPoolRealization>,
    pub requirements: Vec<LoweredPoolResourceRequirement>,
    pub observations: Vec<LoweredPoolObservation>,
}
// Kernel records deliberately have no serialization API. Keep their production
// representation intact; the artifact explicitly enumerates every field.
impl Fixture {
    pub fn new(realizations: u16, resources: u16) -> Self {
        let envelope = (0..realizations)
            .map(|index| LoweredPoolRealization {
                realization: index,
                placement: MemberPlacement {
                    node: NodeId(index),
                    realization: index,
                    play: 1,
                },
                boot: index + 100,
                offer_generation: 1,
                capability: index + 200,
                implementation: index + 300,
                artifact: index + 400,
                member_capacity: 1,
            })
            .collect::<Vec<_>>();
        let mut requirements = Vec::new();
        let mut observations = Vec::new();
        for candidate in &envelope {
            for resource in 0..resources {
                requirements.push(LoweredPoolResourceRequirement {
                    realization: candidate.realization,
                    resource,
                    units: 4,
                });
                observations.push(LoweredPoolObservation {
                    realization: candidate.realization,
                    resource,
                    boot: candidate.boot,
                    offer_generation: candidate.offer_generation,
                    capability: candidate.capability,
                    implementation: candidate.implementation,
                    artifact: candidate.artifact,
                    health: LoweredObservationHealth::Ready,
                    unreserved_units: 8 + u32::from(candidate.realization),
                    utilized_units: 2,
                    sign: candidate.realization * resources + resource + 1,
                });
            }
        }
        Self {
            envelope,
            requirements,
            observations,
        }
    }
    pub fn artifact(&self) -> serde_json::Value {
        serde_json::json!({
            "envelope": self.envelope.iter().map(|e| serde_json::json!({"realization":e.realization,"node":e.placement.node.0,"play":e.placement.play,"boot":e.boot,"offer_generation":e.offer_generation,"capability":e.capability,"implementation":e.implementation,"artifact":e.artifact,"member_capacity":e.member_capacity})).collect::<Vec<_>>(),
            "requirements": self.requirements.iter().map(|r| serde_json::json!({"realization":r.realization,"resource":r.resource,"units":r.units})).collect::<Vec<_>>(),
            "observations":self.observations.iter().map(|o|serde_json::json!({"realization":o.realization,"resource":o.resource,"boot":o.boot,"offer_generation":o.offer_generation,"capability":o.capability,"implementation":o.implementation,"artifact":o.artifact,"health":"Ready","unreserved_units":o.unreserved_units,"utilized_units":o.utilized_units,"sign":o.sign})).collect::<Vec<_>>()
        })
    }
    fn select(
        &self,
        pool: &mut FixedSharedPool<1, 4>,
        evidence: &mut [u16; 4],
    ) -> Result<SelectedPoolMember, PoolSelectionError> {
        admit_selected_pool_member(
            pool,
            MemberKey([1; 32]),
            7,
            black_box(&self.envelope),
            black_box(&self.requirements),
            black_box(&self.observations),
            PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
            evidence,
        )
    }
}
#[derive(serde::Serialize)]
pub struct Run {
    pub decisions: u32,
    pub examined_realizations: u64,
    pub selected_observation_signs: u64,
    pub lifecycle_signs: u64,
    pub summed_selection_wall_ns: u64,
    pub latency_ns: Vec<u64>,
    pub latency_distribution: timing::Distribution,
    pub whole_trial_loop: Metrics,
    pub selected_realization: u16,
    pub timed_boundary: &'static str,
}
pub fn run(fixture: &Fixture, config: &Config) -> Result<Run, String> {
    let count = fixture.envelope.len() as u16;
    let resources = fixture.requirements.len() / fixture.envelope.len();
    let mut latency_ns = Vec::with_capacity(config.decisions as usize);
    let mut summed_selection_wall_ns = 0_u64;
    let mut examined = 0_u64;
    let mut signs = 0_u64;
    for _ in 0..1_000 {
        let mut pool = FixedSharedPool::<1, 4>::new(PoolId(1), 1, 7, count)
            .map_err(|e| format!("pool: {e:?}"))?;
        let selected = fixture
            .select(&mut pool, &mut [0; 4])
            .map_err(|e| format!("warmup selection: {e:?}"))?;
        complete(&mut pool, selected)?;
    }
    // Every independent trial starts with fresh admitted finite pool storage.
    // Setup/activation/release are outside the per-selection clock, and inside
    // the separately reported whole-loop wall/CPU/allocation measurement.
    let (result, whole_trial_loop) = timing::measure(|| -> Result<(), String> {
        for _ in 0..config.decisions {
            let mut pool = FixedSharedPool::<1, 4>::new(PoolId(1), 1, 7, count)
                .map_err(|e| format!("pool: {e:?}"))?;
            let mut evidence = [0_u16; 4];
            let start = Instant::now();
            let selected = fixture.select(&mut pool, &mut evidence);
            let elapsed = timing::ns(start.elapsed());
            let selected = selected.map_err(|e| format!("select: {e:?}"))?;
            summed_selection_wall_ns += elapsed;
            latency_ns.push(elapsed);
            if selected.member.placement.realization != count - 1
                || usize::from(selected.observation_sign_count) != resources
                || selected.examined_realizations != count
            {
                return Err("selector result differs from frozen capacity policy".into());
            }
            for (index, sign) in evidence[..resources].iter().enumerate() {
                if *sign != (count - 1) * resources as u16 + index as u16 + 1 {
                    return Err("wrong selected observation Sign".into());
                }
            }
            examined += u64::from(selected.examined_realizations);
            signs += complete(&mut pool, selected)?;
            black_box(pool);
        }
        Ok(())
    });
    result?;
    let latency_distribution = timing::distribution(&latency_ns);
    Ok(Run { decisions: config.decisions, examined_realizations: examined,
        selected_observation_signs: u64::from(config.decisions) * resources as u64,
        lifecycle_signs: signs, summed_selection_wall_ns, latency_ns, latency_distribution,
        whole_trial_loop, selected_realization: count - 1,
        timed_boundary: "production admit_selected_pool_member: validation + observation selection + selected Signs + atomic occupation; fresh-pool setup and lifecycle/receipt validation excluded from per-selection clock, included in whole loop; clocks not subtracted" })
}
fn complete(pool: &mut FixedSharedPool<1, 4>, selected: SelectedPoolMember) -> Result<u64, String> {
    pool.trigger(selected.member)
        .map_err(|e| format!("start: {e:?}"))?;
    pool.request_release(selected.member)
        .map_err(|e| format!("release: {e:?}"))?;
    pool.complete_release(selected.member)
        .map_err(|e| format!("release completion: {e:?}"))?;
    if pool.signs().count() != 4 || pool.population() != 0 {
        return Err("missing lifecycle Sign or released occupation".into());
    }
    Ok(4)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_selection_keeps_exact_scope_and_refuses_stale_or_missing_evidence() {
        let mut fixture = Fixture::new(8, 4);
        let mut pool = FixedSharedPool::<1, 4>::new(PoolId(1), 1, 7, 8).unwrap();
        let selected = fixture.select(&mut pool, &mut [0; 4]).unwrap();
        assert_eq!(selected.member.placement.realization, 7);
        assert_eq!(selected.examined_realizations, 8);
        assert_eq!(pool.signs().count(), 1);
        fixture
            .observations
            .iter_mut()
            .for_each(|o| o.offer_generation += 1);
        let mut pool = FixedSharedPool::<1, 4>::new(PoolId(1), 1, 7, 8).unwrap();
        assert!(matches!(
            fixture.select(&mut pool, &mut [0; 4]),
            Err(PoolSelectionError::NoCurrentRealization { .. })
        ));
        assert_eq!(pool.population(), 0);
        fixture.observations.clear();
        assert!(matches!(
            fixture.select(&mut pool, &mut [0; 4]),
            Err(PoolSelectionError::NoCurrentRealization { .. })
        ));
        assert_eq!(pool.population(), 0);
    }
}
