//! Renderer-neutral explanation of the heterogeneous scheduler capstone.

use conduit_planner::{
    CapstoneGainDimension, HeterogeneousCapstoneReport, SchedulerProofClass, SchedulerStrategy,
};
use serde::{Deserialize, Serialize};

pub const MAX_CAPSTONE_EXPLANATION_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayCapstoneBaseline {
    pub name: String,
    pub gains: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayHeterogeneousCapstoneExplanation {
    pub proof_class: String,
    pub optimized_measurements: Vec<String>,
    pub baselines: Vec<PatchbayCapstoneBaseline>,
    pub placement_decisions: Vec<String>,
    pub intentionally_unused_devices: Vec<String>,
    pub partial_failure: Vec<String>,
    pub physical_evidence_claimed: bool,
}

impl PatchbayHeterogeneousCapstoneExplanation {
    pub fn from_report(report: &HeterogeneousCapstoneReport) -> Result<Self, String> {
        if report.proof_class != SchedulerProofClass::DeterministicFixture {
            return Err("this view explains deterministic capstone evidence only".into());
        }
        let optimized = &report.optimized;
        let optimized_measurements = vec![
            format!("interactive latency={}us", optimized.interactive_latency_us),
            format!(
                "batch throughput={}/s",
                optimized.batch_throughput_items_per_second
            ),
            format!(
                "Line traffic={} bytes/{} messages",
                optimized.line_bytes, optimized.line_messages
            ),
            format!("planner work={}", optimized.planner_work_units),
            format!(
                "accelerator reserved/utilized={}/{}",
                optimized.accelerator_reserved_units, optimized.accelerator_utilized_units
            ),
            format!(
                "fusion={} churn={} pressure={} refusals={}",
                optimized.fusion_choices,
                optimized.placement_churn,
                optimized.pressure_events,
                optimized.refusal_events
            ),
        ];
        let baselines = report
            .gains_by_baseline
            .iter()
            .map(|(strategy, gains)| PatchbayCapstoneBaseline {
                name: strategy_name(*strategy).to_owned(),
                gains: gains
                    .iter()
                    .map(|gain| gain_name(*gain).to_owned())
                    .collect(),
            })
            .collect();
        let placement_decisions = report
            .decisions
            .iter()
            .map(|decision| {
                format!(
                    "{}: {} ({:?})",
                    decision.workload_id, decision.choice, decision.principal_reason
                )
            })
            .collect();
        let intentionally_unused_devices = report
            .devices
            .iter()
            .filter_map(|device| match &device.disposition {
                conduit_planner::CapstoneDeviceDisposition::IntentionallyUnused { reason } => {
                    Some(format!("{}: {reason}", device.class_id))
                }
                conduit_planner::CapstoneDeviceDisposition::Used { .. } => None,
            })
            .collect();
        let partial_failure = report
            .degradation
            .fragments
            .iter()
            .map(|fragment| format!("{}: {:?}", fragment.fragment_id, fragment.disposition))
            .collect();
        let explanation = Self {
            proof_class: "deterministic fixture (not physical or HIL evidence)".into(),
            optimized_measurements,
            baselines,
            placement_decisions,
            intentionally_unused_devices,
            partial_failure,
            physical_evidence_claimed: false,
        };
        let explanation_bytes = explanation.proof_class.len()
            + explanation
                .optimized_measurements
                .iter()
                .map(String::len)
                .sum::<usize>()
            + explanation
                .baselines
                .iter()
                .map(|baseline| {
                    baseline.name.len() + baseline.gains.iter().map(String::len).sum::<usize>()
                })
                .sum::<usize>()
            + explanation
                .placement_decisions
                .iter()
                .chain(&explanation.intentionally_unused_devices)
                .chain(&explanation.partial_failure)
                .map(String::len)
                .sum::<usize>();
        if explanation_bytes > MAX_CAPSTONE_EXPLANATION_BYTES {
            return Err("capstone explanation exceeds its finite bound".into());
        }
        Ok(explanation)
    }
}

fn strategy_name(strategy: SchedulerStrategy) -> &'static str {
    match strategy {
        SchedulerStrategy::Optimized => "optimized",
        SchedulerStrategy::CentralizedStrongestHost => "centralized strongest host",
        SchedulerStrategy::CheapestFitWithoutCoordinationCost => {
            "cheapest fit without coordination cost"
        }
    }
}

fn gain_name(gain: CapstoneGainDimension) -> &'static str {
    match gain {
        CapstoneGainDimension::InteractiveLatency => "interactive latency",
        CapstoneGainDimension::BatchThroughput => "batch throughput",
        CapstoneGainDimension::LineBytes => "Line bytes",
        CapstoneGainDimension::LineMessages => "Line messages",
        CapstoneGainDimension::PlannerWork => "planner work",
        CapstoneGainDimension::PlacementChurn => "placement churn",
        CapstoneGainDimension::CoordinationWork => "coordination work",
    }
}
