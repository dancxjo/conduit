use conduit_core::{
    CommitOrdering, DeterministicCommitFrontier, ExecutionArrangement, ExecutionBoundary,
    ExecutionCommitDomain, ExecutionContractError, ExecutionGuarantee, ExecutionLane,
    ExecutionLogicalCord, ExecutionPlacement, ExecutionProposalTicket, ExecutionRegion, Id,
    InstancePath, IsolationProfile, PinnedDescriptor, SemanticHash, validate_execution_arrangement,
};

const PIN_HASH: SemanticHash = SemanticHash::from_bytes([1; 32]);

fn pin(id: &'static str) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: PIN_HASH,
    }
}

const HOSTS: [Id<'static>; 1] = [Id("host-observation")];

fn placement(isolation: IsolationProfile) -> ExecutionPlacement<'static> {
    ExecutionPlacement {
        id: Id("placement-a"),
        host_observation: HOSTS[0],
        provider: pin("provider/fixed-hosted-lanes"),
        authority_boundary: pin("boundary/authority"),
        resource_boundary: pin("boundary/resources"),
        lifecycle_boundary: pin("boundary/lifecycle"),
        failure_boundary: pin("boundary/failure"),
        generation: 7,
        isolation,
        memory_containment: ExecutionGuarantee::Guaranteed,
        regain_control: ExecutionGuarantee::Guaranteed,
        effect_fencing: ExecutionGuarantee::Guaranteed,
        stop_execution: ExecutionGuarantee::Guaranteed,
        reclaim_resources: ExecutionGuarantee::Guaranteed,
        maximum_regain_control_ticks: 4,
    }
}

fn lane(id: &'static str) -> ExecutionLane<'static> {
    ExecutionLane {
        id: Id(id),
        placement: Id("placement-a"),
        placement_generation: 7,
        generation: 7,
        independent_progress: ExecutionGuarantee::Guaranteed,
        simultaneous_execution: ExecutionGuarantee::Guaranteed,
        preemption: ExecutionGuarantee::Observed,
        termination: ExecutionGuarantee::Observed,
        ready_slots: 1,
        wake_slots: 2,
        proposal_slots: 1,
        commit_slots: 1,
        timer_slots: 1,
        scratch_bytes: 64,
        stack_bytes: 4096,
        evidence_slots: 8,
    }
}

fn region<'a>(
    id: &'static str,
    lane: &'static str,
    members: &'a [InstancePath<'a>],
) -> ExecutionRegion<'a> {
    ExecutionRegion {
        id: Id(id),
        members,
        placement: Id("placement-a"),
        placement_generation: 7,
        lane: Id(lane),
        lane_generation: 7,
        commit_domain: Id("commit-main"),
        independent: true,
        maximum_in_flight_proposals: 1,
        scratch_bytes: 64,
        retained_state_bytes: 64,
        pending_operation_slots: 1,
        timer_slots: 1,
        evidence_slots: 8,
    }
}

fn domain() -> ExecutionCommitDomain<'static> {
    ExecutionCommitDomain {
        id: Id("commit-main"),
        ordering: CommitOrdering::DeterministicFrontier,
        proposal_slots: 3,
        commit_slots: 3,
        maximum_proposal_bytes: 1024,
        maximum_head_of_line_ticks: 20,
        cancellation_slots: 3,
        evidence_slots: 32,
    }
}

fn boundaries<'a>(cords: &'a [ExecutionLogicalCord<'a>; 2]) -> [ExecutionBoundary<'a>; 2] {
    [
        ExecutionBoundary {
            cord: cords[0].id,
            from_region: Id("region-source"),
            to_region: Id("region-transform"),
            realization: pin("boundary/in-address-space"),
            generation: 7,
            from_placement_generation: 7,
            to_placement_generation: 7,
            capacity_items: 2,
            capacity_bytes: 128,
            wake_slots: 2,
            evidence_slots: 4,
        },
        ExecutionBoundary {
            cord: cords[1].id,
            from_region: Id("region-transform"),
            to_region: Id("region-sink"),
            realization: pin("boundary/in-address-space"),
            generation: 7,
            from_placement_generation: 7,
            to_placement_generation: 7,
            capacity_items: 2,
            capacity_bytes: 128,
            wake_slots: 2,
            evidence_slots: 4,
        },
    ]
}

#[derive(Clone, Copy)]
enum Case {
    Valid,
    ObservedSimultaneity,
    BoundaryCapacityMismatch,
    IncompleteTermination,
    StaleLaneGeneration,
    CommitCapacityExceeded,
}

fn validate_case(case: Case) -> Result<(), ExecutionContractError> {
    let nodes = [
        InstancePath::new("source").unwrap(),
        InstancePath::new("transform").unwrap(),
        InstancePath::new("sink").unwrap(),
    ];
    let cords = [
        ExecutionLogicalCord {
            id: Id("source-to-transform"),
            from: nodes[0],
            to: nodes[1],
            capacity_items: 2,
            capacity_bytes: 128,
        },
        ExecutionLogicalCord {
            id: Id("transform-to-sink"),
            from: nodes[1],
            to: nodes[2],
            capacity_items: 2,
            capacity_bytes: 128,
        },
    ];
    let mut selected_placement = placement(if matches!(case, Case::IncompleteTermination) {
        IsolationProfile::IsolatedTerminable
    } else {
        IsolationProfile::StepNative
    });
    if matches!(case, Case::IncompleteTermination) {
        selected_placement.effect_fencing = ExecutionGuarantee::Observed;
    }
    let placements = [selected_placement];
    let mut first_lane = lane("lane-a");
    if matches!(case, Case::ObservedSimultaneity) {
        first_lane.simultaneous_execution = ExecutionGuarantee::Observed;
    }
    if matches!(case, Case::StaleLaneGeneration) {
        first_lane.placement_generation = 6;
    }
    let lanes = [first_lane, lane("lane-b"), lane("lane-c")];
    let source = [nodes[0]];
    let transform = [nodes[1]];
    let sink = [nodes[2]];
    let regions = [
        region("region-source", "lane-a", &source),
        region("region-transform", "lane-b", &transform),
        region("region-sink", "lane-c", &sink),
    ];
    let mut selected_boundaries = boundaries(&cords);
    if matches!(case, Case::BoundaryCapacityMismatch) {
        selected_boundaries[0].capacity_items = 3;
    }
    let mut selected_domain = domain();
    if matches!(case, Case::CommitCapacityExceeded) {
        selected_domain.commit_slots = 2;
    }
    let domains = [selected_domain];
    validate_execution_arrangement(
        ExecutionArrangement {
            placements: &placements,
            lanes: &lanes,
            regions: &regions,
            boundaries: &selected_boundaries,
            commit_domains: &domains,
        },
        &nodes,
        &cords,
        &HOSTS,
    )
}

#[test]
fn exact_regions_lanes_boundaries_and_commit_reservations_validate() {
    assert_eq!(validate_case(Case::Valid), Ok(()));
}

#[test]
fn observed_parallelism_cannot_satisfy_an_independent_region() {
    assert_eq!(
        validate_case(Case::ObservedSimultaneity),
        Err(ExecutionContractError::InvalidGuarantee)
    );
}

#[test]
fn crossing_cord_requires_one_exact_capacity_preserving_boundary() {
    assert_eq!(
        validate_case(Case::BoundaryCapacityMismatch),
        Err(ExecutionContractError::BoundaryMismatch)
    );
}

#[test]
fn terminable_claim_requires_preemption_fencing_stop_and_reclamation() {
    assert_eq!(
        validate_case(Case::IncompleteTermination),
        Err(ExecutionContractError::InvalidGuarantee)
    );
}

#[test]
fn stale_lane_generation_is_fenced_before_start() {
    assert_eq!(
        validate_case(Case::StaleLaneGeneration),
        Err(ExecutionContractError::GenerationMismatch)
    );
}

#[test]
fn complete_proposal_population_is_reserved_in_the_commit_window() {
    assert_eq!(
        validate_case(Case::CommitCapacityExceeded),
        Err(ExecutionContractError::CapacityExceeded)
    );
}

#[test]
fn conformance_inventory_names_the_portable_execution_contract() {
    let fixture = include_str!("../../../conformance/c4/portable-execution.json");
    for id in [
        "three-independent-regions",
        "observed-simultaneity-rejected",
        "cross-region-boundary-capacity",
        "terminable-profile-complete-guarantees",
        "deterministic-frontier-reserved",
        "generation-fenced-placement",
    ] {
        assert!(fixture.contains(&format!("\"id\":\"{id}\"")));
    }
}

#[test]
fn commit_frontier_ignores_physical_completion_and_fences_stale_proposals() {
    let mut frontier = DeterministicCommitFrontier::new(4, Id("commit-main"), 10, 3).unwrap();
    let ticket = |sequence| ExecutionProposalTicket {
        plan_epoch: 4,
        commit_domain: Id("commit-main"),
        sequence,
    };
    frontier.dispatch(ticket(10)).unwrap();
    frontier.dispatch(ticket(11)).unwrap();
    frontier.dispatch(ticket(12)).unwrap();
    assert_eq!(frontier.in_flight(), 3);
    assert_eq!(
        frontier.commit_head(ticket(12)),
        Err(ExecutionContractError::GenerationMismatch)
    );
    frontier.commit_head(ticket(10)).unwrap();
    frontier.commit_head(ticket(11)).unwrap();
    assert_eq!(frontier.next_commit(), 12);
    assert_eq!(frontier.fence(5), Ok(1));
    assert_eq!(frontier.in_flight(), 0);
    assert_eq!(
        frontier.commit_head(ticket(12)),
        Err(ExecutionContractError::GenerationMismatch)
    );
}
