//! Host/build-only construction of the exact RP2040 reference plan.

use conduit_core::{
    ArtifactDigest, AuthorityTime, BlockingFairness, BoundednessProfile, CancellationGuarantee,
    Direction, ExecutionLimits, ExecutionPlan, ExecutionProfile, FlowCapacity, FlowPolicy,
    FlowWatermarks, HandleDisposition, Id, InstancePath, MemoryAccounting, MemoryCategory,
    MemoryClaim, OwnershipModel, PinnedDescriptor, PlanArtifact, PlanHostObservation,
    PlanResourceBudget, Pressure, ResolvedPlanCord, ResolvedPlanNode, ResolvedPlanPort,
    SemanticHash, TypeContractRef, ValueRepresentation,
};
use conduit_embedded::EmbeddedProfile;

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/sample"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([9; 32]),
};
const CLAIMS: [MemoryClaim; 1] = [MemoryClaim {
    category: MemoryCategory::PortTransactions,
    accounting: MemoryAccounting::ExecutorAllocated,
    bytes: 320,
}];
const REPRESENTATIONS: [ValueRepresentation<'static>; 2] = [
    ValueRepresentation {
        direction: Direction::Input,
        port: Id("in"),
        semantic_type: TYPE,
        representation: PinnedDescriptor {
            id: Id("fixture/fixed-bytes"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([12; 32]),
        },
        ownership: OwnershipModel::Owned,
        disposition: HandleDisposition::None,
        max_bytes: 8,
    },
    ValueRepresentation {
        direction: Direction::Output,
        port: Id("out"),
        semantic_type: TYPE,
        representation: PinnedDescriptor {
            id: Id("fixture/fixed-bytes"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([12; 32]),
        },
        ownership: OwnershipModel::Owned,
        disposition: HandleDisposition::None,
        max_bytes: 8,
    },
];
const LIMITS: ExecutionLimits = ExecutionLimits {
    max_step_work: 4,
    max_retained_values: 0,
    max_retained_bytes: 0,
    max_scratch_bytes: 0,
    max_input_leases: 1,
    max_input_bytes: 8,
    max_output_reservations: 1,
    max_output_bytes: 8,
    max_transactions: 1,
    max_fragments_per_step: 0,
    max_pending_operations: 0,
    max_timers: 0,
    max_child_tasks: 0,
    max_host_buffer_bytes: 0,
    max_foreign_queue_items: 0,
    max_foreign_queue_bytes: 0,
    max_checkpoint_bytes: 0,
    implementation_memory_bytes: 320,
    cancellation_ticks: 8,
};

pub const PROGRAM_FIXTURE_PACKAGE_HASH: SemanticHash = SemanticHash::from_bytes([70; 32]);
pub const PROGRAM_FIXTURE_LOCK_HASH: SemanticHash = SemanticHash::from_bytes([71; 32]);

#[must_use]
pub fn embedded_profile() -> EmbeddedProfile {
    let mut profile = EmbeddedProfile {
        identity: ZERO,
        maximum_nodes: 3,
        maximum_cords: 2,
        maximum_ports: 4,
        maximum_queue_slots: 2,
        maximum_value_bytes: 16,
        maximum_evidence_records: 64,
        maximum_timers: 4,
        maximum_interests_per_node: 4,
        maximum_nesting: 2,
        maximum_timer_delay: 1_000,
        static_ram_budget_bytes: 64 * 1024,
        stack_budget_bytes: 4 * 1024,
        flash_budget_bytes: 96 * 1024,
    };
    profile.seal().expect("static embedded profile");
    profile
}

/// Construct the desktop oracle and exact RP2040 plan from one semantic source.
pub fn with_equivalence_plans<R>(
    action: impl FnOnce(ExecutionPlan<'_>, ExecutionPlan<'_>, &ExecutionProfile<'_>) -> R,
) -> R {
    let profile = execution_profile();
    let desktop_observations = [observation("fixture/desktop-report", "fixture/desktop", 1)];
    let rp2040_observations = [observation("fixture/rp2040-report", "fixture/rp2040", 2)];
    let desktop_artifacts = [
        artifact("fixture/desktop-sensor-artifact", 3),
        artifact("fixture/desktop-threshold-artifact", 4),
        artifact("fixture/desktop-indicator-artifact", 5),
    ];
    let rp2040_artifacts = [
        artifact("fixture/rp2040-sensor-artifact", 6),
        artifact("fixture/rp2040-threshold-artifact", 7),
        artifact("fixture/rp2040-indicator-artifact", 8),
    ];
    let instances = [
        InstancePath::new("fixture/sensor").expect("reference instance"),
        InstancePath::new("fixture/threshold").expect("reference instance"),
        InstancePath::new("fixture/indicator").expect("reference instance"),
    ];
    let desktop_nodes = [
        node(
            instances[0],
            "fixture/sensor",
            20,
            "fixture/desktop-sensor",
            30,
            desktop_artifacts[0].id,
            desktop_observations[0],
            &profile,
        ),
        node(
            instances[1],
            "fixture/threshold",
            21,
            "fixture/desktop-threshold",
            31,
            desktop_artifacts[1].id,
            desktop_observations[0],
            &profile,
        ),
        node(
            instances[2],
            "fixture/indicator",
            22,
            "fixture/desktop-indicator",
            32,
            desktop_artifacts[2].id,
            desktop_observations[0],
            &profile,
        ),
    ];
    let rp2040_nodes = [
        node(
            instances[0],
            "fixture/sensor",
            20,
            "fixture/rp2040-sensor",
            40,
            rp2040_artifacts[0].id,
            rp2040_observations[0],
            &profile,
        ),
        node(
            instances[1],
            "fixture/threshold",
            21,
            "fixture/rp2040-threshold",
            41,
            rp2040_artifacts[1].id,
            rp2040_observations[0],
            &profile,
        ),
        node(
            instances[2],
            "fixture/indicator",
            22,
            "fixture/rp2040-indicator",
            42,
            rp2040_artifacts[2].id,
            rp2040_observations[0],
            &profile,
        ),
    ];
    let capacity = FlowCapacity::new(1, 8, 8).expect("reference flow capacity");
    let flow = FlowPolicy::new(
        capacity,
        Pressure::Block(BlockingFairness::Fifo),
        FlowWatermarks::new(0, 1, capacity).expect("reference watermarks"),
    )
    .expect("reference flow policy");
    let desktop_cords = [
        cord(
            "fixture/sample",
            desktop_nodes[0].instance,
            desktop_nodes[1].instance,
            50,
            flow,
        ),
        cord(
            "fixture/decision",
            desktop_nodes[1].instance,
            desktop_nodes[2].instance,
            51,
            flow,
        ),
    ];
    let rp2040_cords = [
        cord(
            "fixture/sample",
            rp2040_nodes[0].instance,
            rp2040_nodes[1].instance,
            50,
            flow,
        ),
        cord(
            "fixture/decision",
            rp2040_nodes[1].instance,
            rp2040_nodes[2].instance,
            51,
            flow,
        ),
    ];
    let mut desktop_plan = plan(
        &desktop_observations,
        &desktop_artifacts,
        &desktop_nodes,
        &desktop_cords,
    );
    let mut rp2040_plan = plan(
        &rp2040_observations,
        &rp2040_artifacts,
        &rp2040_nodes,
        &rp2040_cords,
    );
    desktop_plan.identity = desktop_plan
        .semantic_hash(&mut [ZERO; 32])
        .expect("desktop reference plan identity");
    rp2040_plan.identity = rp2040_plan
        .semantic_hash(&mut [ZERO; 32])
        .expect("RP2040 reference plan identity");
    assert_eq!(
        desktop_plan.source_semantic_hash,
        rp2040_plan.source_semantic_hash
    );
    assert_ne!(desktop_plan.identity, rp2040_plan.identity);
    action(desktop_plan, rp2040_plan, &profile)
}

fn execution_profile() -> ExecutionProfile<'static> {
    let mut profile = ExecutionProfile {
        id: Id("fixture/embedded-equivalence-profile"),
        schema_version: 0,
        semantic_hash: ZERO,
        boundedness: BoundednessProfile::Hard,
        cancellation: CancellationGuarantee::Bounded,
        step_bound_enforced: true,
        limits: LIMITS,
        representations: &REPRESENTATIONS,
        memory_claims: &CLAIMS,
        checkpoint: None,
    };
    profile.semantic_hash = profile
        .computed_semantic_hash(&mut [ZERO; 3])
        .expect("reference execution profile identity");
    profile
}

fn plan<'a>(
    observations: &'a [PlanHostObservation<'a>],
    artifacts: &'a [PlanArtifact<'a>],
    nodes: &'a [ResolvedPlanNode<'a>],
    cords: &'a [ResolvedPlanCord<'a>],
) -> ExecutionPlan<'a> {
    ExecutionPlan {
        schema_version: 0,
        identity: ZERO,
        source_semantic_hash: hash(60),
        resolver: pin("fixture/resolver", 61),
        resolver_policy_hash: hash(62),
        created_at: AuthorityTime {
            basis: Id("clock/monotonic"),
            tick: 1,
        },
        budget: PlanResourceBudget {
            memory_bytes: 32_000_000,
            storage_bytes: 0,
            cpu_units: 3,
            timers: 0,
            transports: 0,
            checkpoints: 0,
            evidence_bytes: 32_000_000,
        },
        host_observations: observations,
        resources: &[],
        workloads: &[],
        artifacts,
        nodes,
        cords,
        value_envelopes: &[],
        clock_conversions: &[],
        feedback_boundaries: &[],
        distributed_cords: &[],
        fanouts: &[],
        merges: &[],
        event_streams: &[],
        runtime_evidence: None,
        evidence_provider: None,
        watch_admissions: &[],
        jobs: &[],
        satisfaction_proofs: &[],
        authorities: &[],
        hazard_closure: None,
        composites: &[],
        port_groups: &[],
        instance_pools: &[],
        supervisions: &[],
        unresolved: &[],
    }
}

fn artifact(id: &'static str, byte: u8) -> PlanArtifact<'static> {
    PlanArtifact {
        id: Id(id),
        digest: ArtifactDigest::from_bytes([byte; 32]),
    }
}

fn observation(id: &'static str, host: &'static str, byte: u8) -> PlanHostObservation<'static> {
    PlanHostObservation {
        id: Id(id),
        host: Id(host),
        boot_id: Id("fixture/host-boot"),
        semantic_hash: hash(byte),
        time_basis: Id("clock/monotonic"),
        observed_at_tick: 0,
        valid_until_tick: 1_000,
    }
}

#[allow(clippy::too_many_arguments)]
fn node<'a>(
    instance: InstancePath<'a>,
    contract_id: &'static str,
    contract_byte: u8,
    implementation_id: &'static str,
    implementation_byte: u8,
    artifact: Id<'static>,
    observation: PlanHostObservation<'static>,
    profile: &'a ExecutionProfile<'static>,
) -> ResolvedPlanNode<'a> {
    ResolvedPlanNode {
        instance,
        contract: pin(contract_id, contract_byte),
        implementation: pin(implementation_id, implementation_byte),
        lifecycle_policy: pin("fixture/lifecycle", 30),
        execution_profile: Some(profile),
        artifact,
        host_observation: observation.id,
        host: observation.host,
        allocation: PlanResourceBudget {
            memory_bytes: 512,
            cpu_units: 1,
            ..PlanResourceBudget::ZERO
        },
        required_resources: &[],
        required_effects: &[],
    }
}

fn cord<'a>(
    id: &'static str,
    from: InstancePath<'a>,
    to: InstancePath<'a>,
    byte: u8,
    flow: FlowPolicy<'a>,
) -> ResolvedPlanCord<'a> {
    ResolvedPlanCord {
        id: Id(id),
        from: ResolvedPlanPort {
            node: from,
            port: Id("out"),
            direction: Direction::Output,
            port_contract_hash: hash(byte),
            value_type: TYPE,
        },
        to: ResolvedPlanPort {
            node: to,
            port: Id("in"),
            direction: Direction::Input,
            port_contract_hash: hash(byte + 20),
            value_type: TYPE,
        },
        flow,
        queue_memory_bytes: 8,
    }
}

const fn hash(byte: u8) -> SemanticHash {
    SemanticHash::from_bytes([byte; 32])
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: hash(byte),
    }
}
