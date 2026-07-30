//! Hosted witnesses for the portable workload admission contract.

use std::time::Instant;

use conduit_core::{
    AuthorityTime, Id, SemanticHash, WorkloadBudget, WorkloadCapability, WorkloadContract,
    WorkloadEvidenceKind, WorkloadLimit, WorkloadPhase, WorkloadReason, WorkloadState,
    WorkloadUsage, admit_workload,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadRunEvidence {
    pub admission: Result<(), WorkloadReason>,
    pub terminal: Option<WorkloadReason>,
    pub phase: Option<WorkloadPhase>,
}

/// Deterministic witness used by conformance without consulting a host clock.
pub fn run_deterministic_workload(
    contract: WorkloadContract<'_>,
    capability: WorkloadCapability<'_>,
    expected_observation: Id<'_>,
    admitted_at: AuthorityTime<'_>,
    usage: &[WorkloadUsage],
    completed_at: AuthorityTime<'_>,
    observed_jitter_ticks: u64,
) -> WorkloadRunEvidence {
    let admission = match admit_workload(contract, capability, expected_observation, admitted_at) {
        Ok(admission) => admission,
        Err(reason) => {
            return WorkloadRunEvidence {
                admission: Err(reason),
                terminal: Some(reason),
                phase: None,
            };
        }
    };
    let mut state = WorkloadState::new(contract, admission);
    for item in usage {
        if let Err(reason) = state.record_usage(*item) {
            return WorkloadRunEvidence {
                admission: Ok(()),
                terminal: Some(reason),
                phase: Some(state.phase()),
            };
        }
    }
    let terminal = state.complete(completed_at, observed_jitter_ticks).err();
    WorkloadRunEvidence {
        admission: Ok(()),
        terminal,
        phase: Some(state.phase()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinuxWorkloadObservation<'a> {
    pub capability: WorkloadCapability<'a>,
    pub process_id: u32,
    pub observed_descriptors: Option<u64>,
    pub elapsed_nanos: u64,
}

/// Measures this process without claiming scheduler, kernel, or device
/// enforcement. The result is deliberately tagged `Measurement`.
pub fn observe_linux_workload<'a>(
    capability_id: Id<'a>,
    host_observation: Id<'a>,
    time_basis: Id<'a>,
    observed_at_tick: u64,
    valid_until_tick: u64,
) -> LinuxWorkloadObservation<'a> {
    let started = Instant::now();
    let observed_descriptors = std::fs::read_dir("/proc/self/fd")
        .ok()
        .and_then(|entries| u64::try_from(entries.count()).ok());
    let process_id = std::process::id();
    let elapsed_nanos = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    LinuxWorkloadObservation {
        capability: WorkloadCapability {
            id: capability_id,
            identity: linux_observation_identity(process_id, elapsed_nanos),
            host_observation,
            evidence_kind: WorkloadEvidenceKind::Measurement,
            time_basis,
            observed_at_tick,
            valid_until_tick,
            capacity: WorkloadBudget {
                work_units: WorkloadLimit::Unsupported,
                tasks: WorkloadLimit::Unsupported,
                processes: WorkloadLimit::Finite(1),
                descriptors: observed_descriptors
                    .map_or(WorkloadLimit::Unsupported, WorkloadLimit::Finite),
                connections: WorkloadLimit::Unsupported,
                storage_bytes: WorkloadLimit::Unsupported,
                device_operations: WorkloadLimit::Unsupported,
                network_bytes: WorkloadLimit::Unsupported,
                callbacks: WorkloadLimit::Unsupported,
                foreign_queue_items: WorkloadLimit::Unsupported,
                transition_overlap_work_units: WorkloadLimit::Unsupported,
            },
            maximum_deadline_ticks: 0,
            maximum_jitter_ticks: 0,
        },
        process_id,
        observed_descriptors,
        elapsed_nanos,
    }
}

fn linux_observation_identity(process_id: u32, elapsed_nanos: u64) -> SemanticHash {
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&process_id.to_be_bytes());
    bytes[4..12].copy_from_slice(&elapsed_nanos.to_be_bytes());
    SemanticHash::from_bytes(bytes)
}
