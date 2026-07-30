use conduit_core::{
    AuthorityTime, DeadlineContract, Id, InstancePath, SemanticHash,
    WORKLOAD_CONTRACT_SCHEMA_VERSION, WorkloadBudget, WorkloadCapability, WorkloadContract,
    WorkloadEvidenceKind, WorkloadGuarantee, WorkloadLimit, WorkloadPhase, WorkloadReason,
    WorkloadState, WorkloadUsage, admit_workload,
};

const CLOCK: Id<'static> = Id("clock/deterministic");
const OBSERVATION: Id<'static> = Id("observation/host");

fn budget(work: u64) -> WorkloadBudget {
    WorkloadBudget {
        work_units: WorkloadLimit::Finite(work),
        tasks: WorkloadLimit::Finite(2),
        processes: WorkloadLimit::Unsupported,
        descriptors: WorkloadLimit::Finite(4),
        connections: WorkloadLimit::Finite(2),
        storage_bytes: WorkloadLimit::Finite(1024),
        device_operations: WorkloadLimit::Unsupported,
        network_bytes: WorkloadLimit::Finite(4096),
        callbacks: WorkloadLimit::Finite(4),
        foreign_queue_items: WorkloadLimit::Finite(2),
        transition_overlap_work_units: WorkloadLimit::Finite(20),
    }
}

fn contract(guarantee: WorkloadGuarantee) -> WorkloadContract<'static> {
    WorkloadContract {
        schema_version: WORKLOAD_CONTRACT_SCHEMA_VERSION,
        id: Id("workload/request"),
        service: Id("service/request"),
        node: InstancePath::new("root/request").unwrap(),
        guarantee,
        budget: budget(100),
        deadline: Some(DeadlineContract {
            time_basis: CLOCK,
            relative_deadline_ticks: 20,
            maximum_jitter_ticks: 2,
        }),
        maximum_evidence_events: 4,
    }
}

fn capability(evidence_kind: WorkloadEvidenceKind) -> WorkloadCapability<'static> {
    WorkloadCapability {
        id: Id("capability/deterministic"),
        identity: SemanticHash::from_bytes([7; 32]),
        host_observation: OBSERVATION,
        evidence_kind,
        time_basis: CLOCK,
        observed_at_tick: 5,
        valid_until_tick: 50,
        capacity: budget(200),
        maximum_deadline_ticks: 30,
        maximum_jitter_ticks: 1,
    }
}

#[test]
fn hard_admission_requires_fresh_exact_enforcement() {
    let now = AuthorityTime {
        basis: CLOCK,
        tick: 10,
    };
    let admission = admit_workload(
        contract(WorkloadGuarantee::Hard),
        capability(WorkloadEvidenceKind::ExactEnforcement),
        OBSERVATION,
        now,
    )
    .expect("exact enforcement admits");
    assert_eq!(admission.absolute_deadline_tick, Some(30));

    assert_eq!(
        admit_workload(
            contract(WorkloadGuarantee::Hard),
            capability(WorkloadEvidenceKind::HostObservation),
            OBSERVATION,
            now,
        ),
        Err(WorkloadReason::ExactEnforcementRequired)
    );
}

#[test]
fn benchmark_never_becomes_admission_authority() {
    let result = admit_workload(
        contract(WorkloadGuarantee::Measured),
        capability(WorkloadEvidenceKind::Benchmark),
        OBSERVATION,
        AuthorityTime {
            basis: CLOCK,
            tick: 10,
        },
    );
    assert_eq!(result, Err(WorkloadReason::BenchmarkIsNotAuthority));
}

#[test]
fn stale_wrong_clock_and_unsupported_profiles_fail_closed() {
    let mut stale = capability(WorkloadEvidenceKind::ExactEnforcement);
    stale.valid_until_tick = 10;
    assert_eq!(
        admit_workload(
            contract(WorkloadGuarantee::Hard),
            stale,
            OBSERVATION,
            AuthorityTime {
                basis: CLOCK,
                tick: 10
            },
        ),
        Err(WorkloadReason::StaleObservation)
    );

    let mut unsupported = contract(WorkloadGuarantee::Unsupported);
    unsupported.deadline = None;
    assert_eq!(
        admit_workload(
            unsupported,
            capability(WorkloadEvidenceKind::None),
            OBSERVATION,
            AuthorityTime {
                basis: CLOCK,
                tick: 10
            },
        ),
        Err(WorkloadReason::UnsupportedWorkload)
    );
}

#[test]
fn overload_and_deadline_miss_are_terminal() {
    let now = AuthorityTime {
        basis: CLOCK,
        tick: 10,
    };
    let declaration = contract(WorkloadGuarantee::Hard);
    let admission = admit_workload(
        declaration,
        capability(WorkloadEvidenceKind::ExactEnforcement),
        OBSERVATION,
        now,
    )
    .unwrap();
    let mut overloaded = WorkloadState::new(declaration, admission);
    assert_eq!(
        overloaded.record_usage(WorkloadUsage {
            callbacks: 5,
            ..WorkloadUsage::default()
        }),
        Err(WorkloadReason::Overload)
    );
    assert_eq!(
        overloaded.phase(),
        WorkloadPhase::Terminal(WorkloadReason::Overload)
    );

    let mut late = WorkloadState::new(declaration, admission);
    assert_eq!(
        late.observe_tick(AuthorityTime {
            basis: CLOCK,
            tick: 30,
        }),
        Err(WorkloadReason::DeadlineMissed)
    );
}

#[test]
fn hidden_foreign_queue_and_overlap_usage_are_accounted() {
    let declaration = contract(WorkloadGuarantee::Hard);
    let admission = admit_workload(
        declaration,
        capability(WorkloadEvidenceKind::ExactEnforcement),
        OBSERVATION,
        AuthorityTime {
            basis: CLOCK,
            tick: 10,
        },
    )
    .unwrap();
    let mut state = WorkloadState::new(declaration, admission);
    state
        .record_usage(WorkloadUsage {
            foreign_queue_items: 2,
            transition_overlap_work_units: 20,
            ..WorkloadUsage::default()
        })
        .unwrap();
    assert_eq!(
        state.record_usage(WorkloadUsage {
            foreign_queue_items: 1,
            ..WorkloadUsage::default()
        }),
        Err(WorkloadReason::Overload)
    );
}

#[test]
fn undeclared_thread_or_priority_escalation_has_no_admission_path() {
    let mut escalated = contract(WorkloadGuarantee::Hard);
    escalated.budget.tasks = WorkloadLimit::Finite(3);
    assert_eq!(
        admit_workload(
            escalated,
            capability(WorkloadEvidenceKind::ExactEnforcement),
            OBSERVATION,
            AuthorityTime {
                basis: CLOCK,
                tick: 10,
            },
        ),
        Err(WorkloadReason::CapacityExceeded)
    );
}
