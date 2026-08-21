//! Finite multidimensional acceptance for the heterogeneous scheduler capstone.

use crate::{DegradationAssessment, PlannerError};
use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const MAXIMUM_CAPSTONE_DECISIONS: usize = 16;
pub const MAXIMUM_CAPSTONE_DEVICE_CLASSES: usize = 16;
pub const MAXIMUM_CAPSTONE_ID_BYTES: usize = 256;
pub const MAXIMUM_CAPSTONE_REASON_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerProofClass {
    DeterministicFixture,
    Physical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchedulerStrategy {
    Optimized,
    CentralizedStrongestHost,
    CheapestFitWithoutCoordinationCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapstoneMeasurement {
    pub strategy: SchedulerStrategy,
    pub semantic_identity: String,
    pub authority_identity: String,
    pub resource_admission_complete: bool,
    pub useful_work_units: u64,
    pub interactive_latency_us: u64,
    pub batch_throughput_items_per_second: u64,
    pub line_bytes: u64,
    pub line_messages: u64,
    pub planner_work_units: u64,
    pub coordination_work_units: u64,
    pub scheduler_overhead_work_units: u64,
    pub accelerator_reserved_units: u64,
    pub accelerator_utilized_units: u64,
    pub fusion_choices: u32,
    pub placement_churn: u32,
    pub pressure_events: u32,
    pub refusal_events: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapstoneDeviceDisposition {
    Used { workload_id: String },
    IntentionallyUnused { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapstoneDeviceClass {
    pub class_id: String,
    pub disposition: CapstoneDeviceDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapstoneDecision {
    pub workload_id: String,
    pub choice: String,
    pub principal_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapstoneGainDimension {
    InteractiveLatency,
    BatchThroughput,
    LineBytes,
    LineMessages,
    PlannerWork,
    CoordinationWork,
    PlacementChurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeterogeneousCapstoneEvidence {
    pub proof_class: SchedulerProofClass,
    pub measurements: Vec<CapstoneMeasurement>,
    pub devices: Vec<CapstoneDeviceClass>,
    pub decisions: Vec<CapstoneDecision>,
    pub cold_replan_result_identity: String,
    pub incremental_replan_result_identity: String,
    pub cold_replan_work_units: u64,
    pub incremental_replan_work_units: u64,
    pub degradation: DegradationAssessment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeterogeneousCapstoneReport {
    pub proof_class: SchedulerProofClass,
    pub optimized: CapstoneMeasurement,
    pub baselines: Vec<CapstoneMeasurement>,
    pub gains_by_baseline: Vec<(SchedulerStrategy, Vec<CapstoneGainDimension>)>,
    pub devices: Vec<CapstoneDeviceClass>,
    pub decisions: Vec<CapstoneDecision>,
    pub cold_replan_work_units: u64,
    pub incremental_replan_work_units: u64,
    pub degradation: DegradationAssessment,
}

pub fn evaluate_heterogeneous_capstone(
    evidence: HeterogeneousCapstoneEvidence,
) -> Result<HeterogeneousCapstoneReport, PlannerError> {
    validate_evidence_shape(&evidence)?;
    let optimized = evidence
        .measurements
        .iter()
        .find(|measurement| measurement.strategy == SchedulerStrategy::Optimized)
        .expect("validated strategy set contains optimized")
        .clone();
    let baselines = evidence
        .measurements
        .iter()
        .filter(|measurement| measurement.strategy != SchedulerStrategy::Optimized)
        .cloned()
        .collect::<Vec<_>>();

    for measurement in &evidence.measurements {
        if measurement.semantic_identity != optimized.semantic_identity
            || measurement.authority_identity != optimized.authority_identity
            || measurement.useful_work_units != optimized.useful_work_units
            || !measurement.resource_admission_complete
        {
            return invalid(
                "capstone strategies do not preserve equal semantics, authority, work, and admission",
            );
        }
        if measurement.accelerator_utilized_units > measurement.accelerator_reserved_units {
            return invalid("capstone accelerator utilization exceeds exact reservation");
        }
    }

    let mut gains_by_baseline = Vec::with_capacity(baselines.len());
    for baseline in &baselines {
        let gains = gains(&optimized, baseline);
        if gains.len() < 2 {
            return invalid(
                "optimized capstone does not improve two distinct dimensions over every baseline",
            );
        }
        let saved_coordination = baseline
            .coordination_work_units
            .checked_sub(optimized.coordination_work_units)
            .ok_or_else(|| {
                PlannerError::InvalidRealizationPolicy(
                    "optimized capstone increases coordination work".into(),
                )
            })?;
        if saved_coordination == 0 || optimized.scheduler_overhead_work_units >= saved_coordination
        {
            return invalid("scheduler overhead is not below the coordination work it saves");
        }
        gains_by_baseline.push((baseline.strategy, gains));
    }

    if evidence.cold_replan_result_identity != evidence.incremental_replan_result_identity
        || evidence.cold_replan_result_identity.is_empty()
        || evidence.incremental_replan_work_units >= evidence.cold_replan_work_units
    {
        return invalid(
            "incremental replan must match the cold oracle with strictly less planning work",
        );
    }
    if evidence.degradation.what_failed().is_empty()
        || evidence.degradation.what_still_works().is_empty()
        || evidence.degradation.automatic_retry_count != 0
    {
        return invalid("capstone partial loss is not scoped or claims an implicit retry");
    }

    Ok(HeterogeneousCapstoneReport {
        proof_class: evidence.proof_class,
        optimized,
        baselines,
        gains_by_baseline,
        devices: evidence.devices,
        decisions: evidence.decisions,
        cold_replan_work_units: evidence.cold_replan_work_units,
        incremental_replan_work_units: evidence.incremental_replan_work_units,
        degradation: evidence.degradation,
    })
}

fn validate_evidence_shape(evidence: &HeterogeneousCapstoneEvidence) -> Result<(), PlannerError> {
    let strategies = evidence
        .measurements
        .iter()
        .map(|measurement| measurement.strategy)
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        SchedulerStrategy::Optimized,
        SchedulerStrategy::CentralizedStrongestHost,
        SchedulerStrategy::CheapestFitWithoutCoordinationCost,
    ]);
    if strategies != expected || evidence.measurements.len() != expected.len() {
        return invalid_unit("capstone requires each exact reviewed strategy once");
    }
    if evidence.devices.is_empty()
        || evidence.devices.len() > MAXIMUM_CAPSTONE_DEVICE_CLASSES
        || evidence.decisions.is_empty()
        || evidence.decisions.len() > MAXIMUM_CAPSTONE_DECISIONS
    {
        return invalid_unit("capstone device or decision count violates its finite bound");
    }
    let mut device_ids = BTreeSet::new();
    let mut used = 0;
    let mut unused = 0;
    for device in &evidence.devices {
        validate_id(&device.class_id)?;
        if !device_ids.insert(device.class_id.as_str()) {
            return invalid_unit("capstone device class identity is duplicated");
        }
        match &device.disposition {
            CapstoneDeviceDisposition::Used { workload_id } => {
                validate_id(workload_id)?;
                used += 1;
            }
            CapstoneDeviceDisposition::IntentionallyUnused { reason } => {
                validate_reason(reason)?;
                unused += 1;
            }
        }
    }
    if used < 3 || unused == 0 {
        return invalid_unit(
            "capstone must selectively use heterogeneous classes and intentionally omit one",
        );
    }
    let mut workloads = BTreeSet::new();
    for decision in &evidence.decisions {
        validate_id(&decision.workload_id)?;
        validate_id(&decision.choice)?;
        validate_reason(&decision.principal_reason)?;
        if !workloads.insert(decision.workload_id.as_str()) {
            return invalid_unit("capstone workload decision identity is duplicated");
        }
    }
    for measurement in &evidence.measurements {
        validate_id(&measurement.semantic_identity)?;
        validate_id(&measurement.authority_identity)?;
        if measurement.useful_work_units == 0
            || measurement.interactive_latency_us == 0
            || measurement.batch_throughput_items_per_second == 0
            || measurement.planner_work_units == 0
            || measurement.coordination_work_units == 0
        {
            return invalid_unit("capstone required measurements must be nonzero");
        }
    }
    Ok(())
}

fn gains(
    optimized: &CapstoneMeasurement,
    baseline: &CapstoneMeasurement,
) -> Vec<CapstoneGainDimension> {
    let mut gains = Vec::new();
    if optimized.interactive_latency_us < baseline.interactive_latency_us {
        gains.push(CapstoneGainDimension::InteractiveLatency);
    }
    if optimized.batch_throughput_items_per_second > baseline.batch_throughput_items_per_second {
        gains.push(CapstoneGainDimension::BatchThroughput);
    }
    if optimized.line_bytes < baseline.line_bytes {
        gains.push(CapstoneGainDimension::LineBytes);
    }
    if optimized.line_messages < baseline.line_messages {
        gains.push(CapstoneGainDimension::LineMessages);
    }
    if optimized.planner_work_units < baseline.planner_work_units {
        gains.push(CapstoneGainDimension::PlannerWork);
    }
    if optimized.coordination_work_units < baseline.coordination_work_units {
        gains.push(CapstoneGainDimension::CoordinationWork);
    }
    if optimized.placement_churn < baseline.placement_churn {
        gains.push(CapstoneGainDimension::PlacementChurn);
    }
    gains
}

fn validate_id(value: &str) -> Result<(), PlannerError> {
    if value.is_empty() || value.len() > MAXIMUM_CAPSTONE_ID_BYTES {
        return invalid_unit("capstone identity is empty or exceeds its finite bound");
    }
    Ok(())
}

fn validate_reason(value: &str) -> Result<(), PlannerError> {
    if value.is_empty() || value.len() > MAXIMUM_CAPSTONE_REASON_BYTES {
        return invalid_unit("capstone reason is empty or exceeds its finite bound");
    }
    Ok(())
}

fn invalid(message: &str) -> Result<HeterogeneousCapstoneReport, PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(message.to_string()))
}

fn invalid_unit(message: &str) -> Result<(), PlannerError> {
    Err(PlannerError::InvalidRealizationPolicy(message.to_string()))
}
