use conduit_core::{
    AuthorityTime, DeadlineContract, Id, InstancePath, SemanticHash,
    WORKLOAD_CONTRACT_SCHEMA_VERSION, WorkloadBudget, WorkloadCapability, WorkloadContract,
    WorkloadEvidenceKind, WorkloadGuarantee, WorkloadLimit, WorkloadPhase, WorkloadReason,
    WorkloadUsage, admit_workload,
};
use conduit_runtime::{observe_linux_workload, run_deterministic_workload};

const CLOCK: Id<'static> = Id("clock/workload");
const OBSERVATION: Id<'static> = Id("observation/workload");

fn budget(work: u64) -> WorkloadBudget {
    WorkloadBudget {
        work_units: WorkloadLimit::Finite(work),
        tasks: WorkloadLimit::Finite(1),
        processes: WorkloadLimit::Unsupported,
        descriptors: WorkloadLimit::Unsupported,
        connections: WorkloadLimit::Unsupported,
        storage_bytes: WorkloadLimit::Unsupported,
        device_operations: WorkloadLimit::Unsupported,
        network_bytes: WorkloadLimit::Unsupported,
        callbacks: WorkloadLimit::Finite(2),
        foreign_queue_items: WorkloadLimit::Finite(1),
        transition_overlap_work_units: WorkloadLimit::Finite(10),
    }
}

fn contract() -> WorkloadContract<'static> {
    WorkloadContract {
        schema_version: WORKLOAD_CONTRACT_SCHEMA_VERSION,
        id: Id("workload/deterministic"),
        service: Id("service/deterministic"),
        node: InstancePath::new("root/work").unwrap(),
        guarantee: WorkloadGuarantee::Hard,
        budget: budget(10),
        deadline: Some(DeadlineContract {
            time_basis: CLOCK,
            relative_deadline_ticks: 5,
            maximum_jitter_ticks: 1,
        }),
        maximum_evidence_events: 4,
    }
}

fn capability() -> WorkloadCapability<'static> {
    WorkloadCapability {
        id: Id("capability/deterministic"),
        identity: SemanticHash::from_bytes([9; 32]),
        host_observation: OBSERVATION,
        evidence_kind: WorkloadEvidenceKind::ExactEnforcement,
        time_basis: CLOCK,
        observed_at_tick: 0,
        valid_until_tick: 20,
        capacity: budget(20),
        maximum_deadline_ticks: 10,
        maximum_jitter_ticks: 1,
    }
}

#[test]
fn deterministic_overload_and_deadline_witnesses_are_exact() {
    let overloaded = run_deterministic_workload(
        contract(),
        capability(),
        OBSERVATION,
        AuthorityTime {
            basis: CLOCK,
            tick: 1,
        },
        &[WorkloadUsage {
            callbacks: 3,
            ..WorkloadUsage::default()
        }],
        AuthorityTime {
            basis: CLOCK,
            tick: 2,
        },
        0,
    );
    assert_eq!(overloaded.terminal, Some(WorkloadReason::Overload));
    assert_eq!(
        overloaded.phase,
        Some(WorkloadPhase::Terminal(WorkloadReason::Overload))
    );

    let late = run_deterministic_workload(
        contract(),
        capability(),
        OBSERVATION,
        AuthorityTime {
            basis: CLOCK,
            tick: 1,
        },
        &[],
        AuthorityTime {
            basis: CLOCK,
            tick: 6,
        },
        0,
    );
    assert_eq!(late.terminal, Some(WorkloadReason::DeadlineMissed));
}

#[test]
fn linux_observation_is_measurement_not_hard_authority() {
    let observed = observe_linux_workload(
        Id("capability/linux-measurement"),
        OBSERVATION,
        CLOCK,
        1,
        10,
    );
    assert_eq!(
        observed.capability.evidence_kind,
        WorkloadEvidenceKind::Measurement
    );
    assert!(observed.process_id > 0);
    assert_eq!(
        admit_workload(
            contract(),
            observed.capability,
            OBSERVATION,
            AuthorityTime {
                basis: CLOCK,
                tick: 2,
            },
        ),
        Err(WorkloadReason::CapacityExceeded)
    );

    let mut superficially_sufficient = observed.capability;
    superficially_sufficient.capacity = budget(20);
    superficially_sufficient.maximum_deadline_ticks = 10;
    superficially_sufficient.maximum_jitter_ticks = 1;
    assert_eq!(
        admit_workload(
            contract(),
            superficially_sufficient,
            OBSERVATION,
            AuthorityTime {
                basis: CLOCK,
                tick: 2,
            },
        ),
        Err(WorkloadReason::ExactEnforcementRequired)
    );
}
