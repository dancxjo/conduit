use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use conduit_core::{
    CommitOrdering, ExecutionArrangement, ExecutionCommitDomain, ExecutionGuarantee, ExecutionLane,
    ExecutionPlacement, ExecutionRegion, Id, InstancePath, IsolationProfile, PinnedDescriptor,
    SemanticHash,
};
use conduit_runtime::{
    FIXED_HOSTED_LANE_PROVIDER_ID, FixedHostedExecutionCoordinator, FixedHostedLaneProvider,
    HostedLaneAssignment, HostedLaneError, HostedLaneJob, HostedLaneReservation,
    ResolvedExecutionArrangement, ResolvedExecutionCommitDomain, ResolvedExecutionDescriptor,
    ResolvedExecutionLane, ResolvedExecutionPlacement, ResolvedExecutionRegion,
};

struct Job {
    value: u64,
    delay_ms: u64,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    fault: bool,
}

impl HostedLaneJob for Job {
    type Proposal = u64;

    fn compute(self) -> Self::Proposal {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        if self.fault {
            panic!("fixture lane fault");
        }
        thread::sleep(Duration::from_millis(self.delay_ms));
        self.active.fetch_sub(1, Ordering::SeqCst);
        self.value
    }

    fn proposal_bytes(_proposal: &Self::Proposal) -> u64 {
        u64::try_from(std::mem::size_of::<u64>()).unwrap()
    }
}

fn reservation() -> HostedLaneReservation {
    HostedLaneReservation {
        generation: 9,
        lanes: 3,
        command_slots_per_lane: 1,
        completion_slots: 3,
        proposal_slots: 3,
        maximum_proposal_bytes: 24,
        evidence_slots: 6,
    }
}

fn resolved_pin(id: &str, byte: u8) -> ResolvedExecutionDescriptor {
    ResolvedExecutionDescriptor {
        id: id.to_owned(),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

fn resolved_arrangement() -> ResolvedExecutionArrangement {
    let placement = ResolvedExecutionPlacement {
        id: "placement-hosted".to_owned(),
        host_observation: "host-observation".to_owned(),
        provider: resolved_pin(FIXED_HOSTED_LANE_PROVIDER_ID, 1),
        authority_boundary: resolved_pin("boundary/authority", 2),
        resource_boundary: resolved_pin("boundary/resources", 3),
        lifecycle_boundary: resolved_pin("boundary/lifecycle", 4),
        failure_boundary: resolved_pin("boundary/failure", 5),
        generation: 12,
        isolation: IsolationProfile::StepNative,
        memory_containment: ExecutionGuarantee::Observed,
        regain_control: ExecutionGuarantee::Observed,
        effect_fencing: ExecutionGuarantee::Observed,
        stop_execution: ExecutionGuarantee::Observed,
        reclaim_resources: ExecutionGuarantee::Observed,
        maximum_regain_control_ticks: 0,
    };
    let lanes = (0..3)
        .map(|index| ResolvedExecutionLane {
            id: format!("lane-{}", char::from(b'a' + index)),
            placement: placement.id.clone(),
            placement_generation: placement.generation,
            generation: 1,
            independent_progress: ExecutionGuarantee::Guaranteed,
            simultaneous_execution: ExecutionGuarantee::Guaranteed,
            preemption: ExecutionGuarantee::Observed,
            termination: ExecutionGuarantee::Observed,
            ready_slots: 1,
            wake_slots: 1,
            proposal_slots: 1,
            commit_slots: 1,
            timer_slots: 1,
            scratch_bytes: 64,
            stack_bytes: 4096,
            evidence_slots: 2,
        })
        .collect::<Vec<_>>();
    let regions = lanes
        .iter()
        .enumerate()
        .map(|(index, lane)| ResolvedExecutionRegion {
            id: format!("region-{}", char::from(b'a' + u8::try_from(index).unwrap())),
            members: vec![char::from(b'a' + u8::try_from(index).unwrap()).to_string()],
            placement: placement.id.clone(),
            placement_generation: placement.generation,
            lane: lane.id.clone(),
            lane_generation: lane.generation,
            commit_domain: "commit-main".to_owned(),
            independent: true,
            maximum_in_flight_proposals: 1,
            scratch_bytes: 64,
            retained_state_bytes: 0,
            pending_operation_slots: 0,
            timer_slots: 1,
            evidence_slots: 2,
        })
        .collect();
    let mut arrangement = ResolvedExecutionArrangement {
        identity: SemanticHash::from_bytes([0; 32]),
        plan_identity: SemanticHash::from_bytes([20; 32]),
        resolution_identity: SemanticHash::from_bytes([21; 32]),
        plan_epoch: 9,
        placements: vec![placement],
        lanes,
        regions,
        boundaries: Vec::new(),
        commit_domains: vec![ResolvedExecutionCommitDomain {
            id: "commit-main".to_owned(),
            ordering: CommitOrdering::DeterministicFrontier,
            proposal_slots: 3,
            commit_slots: 3,
            maximum_proposal_bytes: 1024,
            maximum_head_of_line_ticks: 10,
            cancellation_slots: 3,
            evidence_slots: 6,
        }],
    };
    arrangement.identity = arrangement.computed_identity();
    arrangement
}

fn hosted_region<'a>(
    id: &'a str,
    lane: ExecutionLane<'a>,
    members: &'a [InstancePath<'a>],
    placement: ExecutionPlacement<'a>,
) -> ExecutionRegion<'a> {
    ExecutionRegion {
        id: Id(id),
        members,
        placement: placement.id,
        placement_generation: placement.generation,
        lane: lane.id,
        lane_generation: lane.generation,
        commit_domain: Id("commit-main"),
        independent: true,
        maximum_in_flight_proposals: 1,
        scratch_bytes: 64,
        retained_state_bytes: 0,
        pending_operation_slots: 0,
        timer_slots: 1,
        evidence_slots: 2,
    }
}

fn run(delays: [u64; 3]) -> (Vec<u64>, Vec<u64>, usize) {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut provider = FixedHostedLaneProvider::start(reservation()).unwrap();
    let batch = provider
        .compute_proposals(
            delays
                .into_iter()
                .enumerate()
                .map(|(index, delay_ms)| {
                    let ticket = u64::try_from(index).unwrap() + 1;
                    (
                        ticket,
                        Job {
                            value: ticket * 10,
                            delay_ms,
                            active: Arc::clone(&active),
                            peak: Arc::clone(&peak),
                            fault: false,
                        },
                    )
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
    let commits = batch
        .proposals()
        .iter()
        .map(|proposal| proposal.value)
        .collect();
    let physical = batch
        .physical_completion_order()
        .iter()
        .map(|observation| observation.ticket)
        .collect();
    let maximum_entered = batch
        .physical_completion_order()
        .iter()
        .map(|observation| observation.entered_sequence)
        .max()
        .unwrap();
    let release = batch.physical_completion_order()[0].release_sequence;
    let minimum_finished = batch
        .physical_completion_order()
        .iter()
        .map(|observation| observation.finished_sequence)
        .min()
        .unwrap();
    assert!(maximum_entered < release);
    assert!(release < minimum_finished);
    assert!(
        batch
            .physical_completion_order()
            .iter()
            .all(|observation| observation.generation == 9
                && observation.release_sequence == release)
    );
    (commits, physical, peak.load(Ordering::SeqCst))
}

#[test]
fn three_fixed_lanes_enter_before_any_finishes_and_commit_by_ticket() {
    let (commits, physical, peak) = run([30, 20, 10]);
    assert_eq!(commits, [10, 20, 30]);
    assert_eq!(physical, [3, 2, 1]);
    assert_eq!(peak, 3);
}

#[test]
fn adversarial_completion_order_cannot_change_authoritative_order() {
    let first = run([30, 20, 10]);
    let second = run([10, 20, 30]);
    assert_ne!(first.1, second.1);
    assert_eq!(first.0, second.0);
    assert_eq!(first.0, [10, 20, 30]);
}

#[test]
fn deterministic_tickets_dispatch_to_explicit_physical_lanes() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let job = |value, delay_ms| Job {
        value,
        delay_ms,
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        fault: false,
    };
    let mut coordinator = FixedHostedExecutionCoordinator::admit(
        &resolved_arrangement(),
        "placement-hosted",
        "commit-main",
        1,
    )
    .unwrap();
    let mut committed = Vec::new();
    let batch = coordinator
        .compute_assigned_and_commit(
            [
                HostedLaneAssignment {
                    lane: 2,
                    job: job(100, 30),
                },
                HostedLaneAssignment {
                    lane: 0,
                    job: job(200, 20),
                },
                HostedLaneAssignment {
                    lane: 1,
                    job: job(300, 10),
                },
            ],
            |ticket, value| {
                committed.push((ticket, value));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(committed, [(1, 100), (2, 200), (3, 300)]);
    assert_eq!(batch.committed_tickets, [1, 2, 3]);
    assert_eq!(
        batch
            .physical_completion_order
            .iter()
            .map(|observation| (observation.ticket, observation.lane))
            .collect::<Vec<_>>(),
        [(3, 1), (2, 0), (1, 2)]
    );
    assert_eq!(peak.load(Ordering::SeqCst), 3);
}

#[test]
fn repeated_batches_reuse_the_fixed_provider_storage() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let mut provider = FixedHostedLaneProvider::start(reservation()).unwrap();
    for batch in 0_u64..64 {
        let job = |offset| Job {
            value: batch * 3 + offset,
            delay_ms: 1,
            active: Arc::clone(&active),
            peak: Arc::clone(&peak),
            fault: false,
        };
        let first_ticket = batch * 3 + 1;
        let proposals = provider
            .compute_proposals([
                (first_ticket, job(1)),
                (first_ticket + 1, job(2)),
                (first_ticket + 2, job(3)),
            ])
            .unwrap();
        assert_eq!(proposals.proposals().len(), 3);
        assert_eq!(proposals.proposals()[0].value, batch * 3 + 1);
        assert_eq!(proposals.proposals()[1].value, batch * 3 + 2);
        assert_eq!(proposals.proposals()[2].value, batch * 3 + 3);
        assert_eq!(proposals.physical_completion_order().len(), 3);
    }
    assert_eq!(peak.load(Ordering::SeqCst), 3);
}

#[test]
fn admission_and_ticket_sets_fail_closed() {
    assert_eq!(
        HostedLaneReservation {
            completion_slots: 2,
            ..reservation()
        }
        .validate(),
        Err(HostedLaneError::InvalidReservation)
    );
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let job = |value| Job {
        value,
        delay_ms: 0,
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        fault: false,
    };
    let mut provider = FixedHostedLaneProvider::start(reservation()).unwrap();
    assert_eq!(
        provider
            .compute_proposals(vec![(1, job(1)), (1, job(2)), (3, job(3))])
            .unwrap_err(),
        HostedLaneError::DuplicateTicket
    );
    assert_eq!(
        provider
            .compute_assigned_proposals([(0, 1, job(1)), (0, 2, job(2)), (2, 3, job(3))])
            .unwrap_err(),
        HostedLaneError::InvalidLaneAssignment
    );
}

#[test]
fn provider_admission_consumes_the_portable_arrangement() {
    let placement = ExecutionPlacement {
        id: Id("placement-hosted"),
        host_observation: Id("host-observation"),
        provider: PinnedDescriptor {
            id: Id(FIXED_HOSTED_LANE_PROVIDER_ID),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([1; 32]),
        },
        authority_boundary: PinnedDescriptor {
            id: Id("boundary/authority"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([2; 32]),
        },
        resource_boundary: PinnedDescriptor {
            id: Id("boundary/resources"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([3; 32]),
        },
        lifecycle_boundary: PinnedDescriptor {
            id: Id("boundary/lifecycle"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([4; 32]),
        },
        failure_boundary: PinnedDescriptor {
            id: Id("boundary/failure"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([5; 32]),
        },
        generation: 12,
        isolation: IsolationProfile::StepNative,
        memory_containment: ExecutionGuarantee::Observed,
        regain_control: ExecutionGuarantee::Observed,
        effect_fencing: ExecutionGuarantee::Observed,
        stop_execution: ExecutionGuarantee::Observed,
        reclaim_resources: ExecutionGuarantee::Observed,
        maximum_regain_control_ticks: 0,
    };
    let lane = |id| ExecutionLane {
        id: Id(id),
        placement: placement.id,
        placement_generation: placement.generation,
        generation: 1,
        independent_progress: ExecutionGuarantee::Guaranteed,
        simultaneous_execution: ExecutionGuarantee::Guaranteed,
        preemption: ExecutionGuarantee::Observed,
        termination: ExecutionGuarantee::Observed,
        ready_slots: 1,
        wake_slots: 1,
        proposal_slots: 1,
        commit_slots: 1,
        timer_slots: 1,
        scratch_bytes: 64,
        stack_bytes: 4096,
        evidence_slots: 2,
    };
    let lanes = [lane("lane-a"), lane("lane-b"), lane("lane-c")];
    let nodes = [
        [InstancePath::new("a").unwrap()],
        [InstancePath::new("b").unwrap()],
        [InstancePath::new("c").unwrap()],
    ];
    let regions = [
        hosted_region("region-a", lanes[0], &nodes[0], placement),
        hosted_region("region-b", lanes[1], &nodes[1], placement),
        hosted_region("region-c", lanes[2], &nodes[2], placement),
    ];
    let domains = [ExecutionCommitDomain {
        id: Id("commit-main"),
        ordering: CommitOrdering::DeterministicFrontier,
        proposal_slots: 3,
        commit_slots: 3,
        maximum_proposal_bytes: 1024,
        maximum_head_of_line_ticks: 10,
        cancellation_slots: 3,
        evidence_slots: 6,
    }];
    let arrangement = ExecutionArrangement {
        placements: &[placement],
        lanes: &lanes,
        regions: &regions,
        boundaries: &[],
        commit_domains: &domains,
    };
    let provider: FixedHostedLaneProvider<Job> =
        FixedHostedLaneProvider::admit(arrangement, placement.id, Id("commit-main")).unwrap();
    assert_eq!(provider.reservation().generation, 12);
    assert_eq!(provider.reservation().lanes, 3);
}

#[test]
fn observed_lane_loss_fails_the_fixed_population_closed() {
    let mut provider = FixedHostedLaneProvider::start(reservation()).unwrap();
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let job = |value| Job {
        value,
        delay_ms: 0,
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        fault: false,
    };
    provider.observe_lane_loss(1).unwrap();
    assert_eq!(
        provider
            .compute_proposals([(1, job(1)), (2, job(2)), (3, job(3))])
            .unwrap_err(),
        HostedLaneError::ProviderLost
    );
}

#[test]
fn hosted_provider_owns_its_portable_conformance_cases() {
    let fixture = include_str!("../../../conformance/c4/portable-execution.json");
    for id in [
        "hosted-three-lane-causal-overlap",
        "hosted-adversarial-completion-deterministic-commit",
        "hosted-worker-fault-disposes-proposal",
        "resolved-arrangement-commits-by-ticket",
        "commit-rejection-fences-coordinator",
        "provider-fault-disposes-reserved-slots",
        "coordinator-cancellation-requires-readmission",
        "hosted-repeated-batches-retain-fixed-storage",
        "hosted-explicit-lane-placement-preserves-commit",
        "hosted-observed-lane-loss-fails-closed",
    ] {
        assert!(fixture.contains(&format!("\"id\":\"{id}\"")));
    }
    assert_eq!(
        fixture.matches("\"runner\":\"fixed-hosted-lanes\"").count(),
        10
    );
}

#[test]
fn a_faulted_proposal_is_disposed_and_does_not_poison_the_next_batch() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let job = |value, fault| Job {
        value,
        delay_ms: 0,
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        fault,
    };
    let mut provider = FixedHostedLaneProvider::start(reservation()).unwrap();
    let failure = provider
        .compute_proposals(vec![
            (1, job(1, false)),
            (2, job(2, true)),
            (3, job(3, false)),
        ])
        .unwrap_err();
    assert_eq!(failure, HostedLaneError::WorkerFault { lane: 1, ticket: 2 });
    let recovered = provider
        .compute_proposals(vec![
            (4, job(4, false)),
            (5, job(5, false)),
            (6, job(6, false)),
        ])
        .unwrap();
    assert_eq!(
        recovered
            .proposals()
            .iter()
            .map(|proposal| proposal.value)
            .collect::<Vec<_>>(),
        [4, 5, 6]
    );
}

#[test]
fn resolved_arrangement_coordinator_commits_only_in_ticket_order() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let job = |value, delay_ms| Job {
        value,
        delay_ms,
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        fault: false,
    };
    let mut coordinator = FixedHostedExecutionCoordinator::admit(
        &resolved_arrangement(),
        "placement-hosted",
        "commit-main",
        40,
    )
    .unwrap();
    let mut committed = Vec::new();
    let batch = coordinator
        .compute_and_commit(
            vec![job(400, 30), job(410, 20), job(420, 10)],
            |ticket, value| {
                committed.push((ticket, value));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(batch.committed_tickets, [40, 41, 42]);
    assert_eq!(committed, [(40, 400), (41, 410), (42, 420)]);
    assert_eq!(
        batch
            .physical_completion_order
            .iter()
            .map(|observation| observation.ticket)
            .collect::<Vec<_>>(),
        [42, 41, 40]
    );
    assert_eq!(peak.load(Ordering::SeqCst), 3);
    assert_eq!(coordinator.plan_epoch(), 9);
    assert_eq!(coordinator.commit_domain(), "commit-main");
}

#[test]
fn commit_rejection_and_worker_fault_terminally_fence_the_coordinator() {
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let job = |value, fault| Job {
        value,
        delay_ms: 0,
        active: Arc::clone(&active),
        peak: Arc::clone(&peak),
        fault,
    };
    let mut rejected = FixedHostedExecutionCoordinator::admit(
        &resolved_arrangement(),
        "placement-hosted",
        "commit-main",
        1,
    )
    .unwrap();
    assert_eq!(
        rejected
            .compute_and_commit(
                vec![job(1, false), job(2, false), job(3, false)],
                |ticket, _| (ticket != 2).then_some(()).ok_or(()),
            )
            .unwrap_err(),
        HostedLaneError::CommitRejected
    );
    assert!(rejected.is_terminal());
    assert_eq!(rejected.disposed_slots(), 2);
    assert_eq!(rejected.cancel(), Err(HostedLaneError::CoordinatorTerminal));

    let mut faulted = FixedHostedExecutionCoordinator::admit(
        &resolved_arrangement(),
        "placement-hosted",
        "commit-main",
        1,
    )
    .unwrap();
    assert_eq!(
        faulted
            .compute_and_commit(vec![job(1, false), job(2, true), job(3, false)], |_, _| Ok(
                ()
            ),)
            .unwrap_err(),
        HostedLaneError::WorkerFault { lane: 1, ticket: 2 }
    );
    assert!(faulted.is_terminal());
    assert_eq!(faulted.disposed_slots(), 3);
}

#[test]
fn cancellation_requires_a_newly_admitted_epoch() {
    let mut coordinator: FixedHostedExecutionCoordinator<Job> =
        FixedHostedExecutionCoordinator::admit(
            &resolved_arrangement(),
            "placement-hosted",
            "commit-main",
            1,
        )
        .unwrap();
    coordinator.cancel().unwrap();
    assert!(coordinator.is_terminal());
    assert_eq!(
        coordinator.cancel(),
        Err(HostedLaneError::CoordinatorTerminal)
    );
}
