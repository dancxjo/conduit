//! Empirical timing of the actual typed Planner selector, outside Play.
//! Forecast coordinates and execution states are controlled fixture evidence;
//! their durations are not measurements of physical producer performance.
mod fixture;
#[cfg(test)]
static TEST_MEASUREMENT: std::sync::Mutex<()> = std::sync::Mutex::new(());
use crate::{
    config::Config,
    timing::{self, Metrics},
};
use conduit_core::*;
use conduit_planner::{resource_cost::*, *};
pub use fixture::Fixture;
use std::{hint::black_box, time::Instant};

#[derive(serde::Serialize)]
pub struct Run {
    pub decisions: u32,
    pub candidates: usize,
    pub fixture_preparation: Metrics,
    pub evidence_binding_wall_ns: u64,
    pub summed_selection_wall_ns: u64,
    pub latency_ns: Vec<u64>,
    pub latency_distribution: timing::Distribution,
    pub whole_trial_loop: Metrics,
    pub arithmetic_work_limit_per_decision: u64,
    pub arithmetic_work_spent: u64,
    pub selected_host: String,
    pub validated_candidate_records: u64,
    pub validated_source_signs: u64,
    pub warmup_decisions: u32,
    pub timed_boundary: &'static str,
}
pub fn run(fixture: &Fixture, config: &Config) -> Result<Run, String> {
    let binding_start = Instant::now();
    fixture.prepared(|demand, contexts, timed, forecasts| {
        let evidence_binding_wall_ns = timing::ns(binding_start.elapsed());
        let requirements = HardRealizationRequirements::default();
        let policy = RealizationPolicy::default();
        let select = |work: &mut ExactNumericWork| select_realization_with_costs(
            black_box(demand), CostSelectionInputs {
                hosts: black_box(&fixture.hosts), contexts: black_box(contexts),
                advertisements: &[], requirements: &requirements, observations: black_box(timed),
                tie_policy: &policy, forecasts: black_box(forecasts),
                intent: PerformanceIntent::Interactive, unavailable: UnavailableCostPolicy::RequireKnown,
            }, work).map_err(|e| format!("typed selection: {e:?}"));
        let expected = &fixture.hosts.last().unwrap().host_id;
        let validate = |selected: &CostRealizationSelection<'_>| -> Result<(), String> {
            if !selected.used_cost_preference || &selected.realization.choice.host_id != expected
                || selected.costs.len() != fixture.hosts.len()
                || selected.realization.signs.len() != fixture.hosts.len()
                || selected.costs.iter().filter(|item| item.selected).count() != 1 {
                return Err("typed selection disagrees with frozen exact fulfillment law".into());
            }
            for (index, item) in selected.costs.iter().enumerate() {
                let CostAvailability::Known(forecast) = item.availability else {
                    return Err("fixture cost unexpectedly unavailable".into());
                };
                if item.host_id != &fixture.hosts[index].host_id
                    || forecast.source().sign() != &fixture.forecast_signs[index]
                    || item.current_execution.map(|o| o.source().sign()) != Some(&fixture.current_signs[index])
                    || item.submitted_execution_assumption.map(|s| s.sign()) != Some(&fixture.assumption_signs[index])
                    || item.expected_execution_condition != Some(CostExecutionCondition::Cold)
                    || item.selected != (item.host_id == expected) {
                    return Err("typed decision lost source, condition or selection custody".into());
                }
            }
            Ok(())
        };
        for _ in 0..1000 {
            validate(&select(&mut ExactNumericWork::new(fixture::WORK_LIMIT))?)?;
        }
        let mut latency_ns = Vec::with_capacity(config.decisions as usize);
        let mut summed_selection_wall_ns = 0;
        let mut arithmetic_work_spent = 0;
        let (result, whole_trial_loop) = timing::measure(|| -> Result<(), String> {
            for _ in 0..config.decisions {
                let mut work = ExactNumericWork::new(fixture::WORK_LIMIT);
                let start = Instant::now();
                let selection = select(&mut work);
                let elapsed = timing::ns(start.elapsed());
                let selected = selection?;
                validate(&selected)?;
                summed_selection_wall_ns += elapsed;
                latency_ns.push(elapsed);
                arithmetic_work_spent += selected.arithmetic_work_spent;
                black_box(selected);
            }
            Ok(())
        });
        result?;
        Ok(Run {
            decisions: config.decisions, candidates: fixture.hosts.len(),
            fixture_preparation: fixture.preparation.as_ref().expect("measured fixture").clone(),
            evidence_binding_wall_ns, summed_selection_wall_ns,
            latency_distribution: timing::distribution(&latency_ns), latency_ns,
            whole_trial_loop, arithmetic_work_limit_per_decision: fixture::WORK_LIMIT,
            arithmetic_work_spent, selected_host: expected.as_str().to_owned(),
            validated_candidate_records: u64::from(config.decisions) * fixture.hosts.len() as u64,
            validated_source_signs: u64::from(config.decisions) * fixture.hosts.len() as u64 * 2,
            warmup_decisions: 1000,
            timed_boundary: "production select_realization_with_costs: current source/clock/condition checks, ordinary hard admission, exact interval comparison and returned decision account; evidence preparation, result validation and destruction excluded from per-decision clocks and included where stated in preparation/whole-loop metrics; no timer subtraction",
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_measured_decision_retains_exact_sources_and_known_fulfillment() {
        let _measurement = TEST_MEASUREMENT.lock().unwrap();
        let config = Config {
            output: std::path::PathBuf::new(),
            values: 1_000_000,
            repetitions: 3,
            decisions: 4,
            samples: 1000,
            hosted_pipeline: false,
        };
        for count in [1, 8, 32] {
            let run = run(&Fixture::new(count).unwrap(), &config).unwrap();
            assert_eq!(run.latency_ns.len(), 4);
            assert_eq!(run.validated_candidate_records, 4 * count as u64);
            assert_eq!(run.validated_source_signs, 8 * count as u64);
            assert_eq!(
                run.selected_host,
                format!("cost-baseline/host-{:02}", count - 1)
            );
            assert_eq!(run.arithmetic_work_spent == 0, count == 1);
        }
    }
}
