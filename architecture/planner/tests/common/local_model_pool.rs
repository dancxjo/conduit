//! Shared ordinary checked local-model-pool planning fixture, not live service.
use conduit_core::{
    kind_id, ArtifactId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BootId,
    CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostCallContractId,
    HostId, HostProfileId, ImplementationId, KindIdentity, OfferGeneration, PlannerCapabilityOffer,
    PlannerLimits, PlannerProfileId, PoolMemberLimits, SharedPoolId, PROTOCOL_VERSION,
    SHARED_POOL_ADMIT_AUTHORITY_CONTRACT, SHARED_POOL_ADMIT_HOST_CALL_CONTRACT,
    SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_shared_pools, PlanningOptions,
    SharedPoolPlanningRequirement,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindProjection,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use std::collections::BTreeMap;

const SOURCE: &str = r#"
plot llm/generate (
    maximum-input-bytes: Count = 4096
    maximum-context-items: Count = 1
    maximum-output-bytes: Count = 1024
    maximum-work-units: Count = 4096
    maximum-history-items: Count = 0
    request: llm/generation-request@1 >> result: llm/generated-result@1
) {
}

plot consumer (
    workers: Pool
) {
    observe: flow/pool-observe(workers)
}

plot model-service {
    pool workers: llm/generate(size = 2)
    client: consumer(workers)
}
"#;

pub(super) fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "flow/pool-observe".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "workers".into(),
                value_type: "Pool".into(),
                default: None,
            }],
        })
        .unwrap();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("flow/pool-observe"),
            kind_contract_revision: KindIdentity::from("flow/pool-observe@1"),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    (startup, profile)
}

pub(super) fn expanded() -> conduit_plot::ExpandedCanonicalPlot {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    expand_canonical_plot(&checked, "model-service", &profile).unwrap()
}

pub(super) fn consumer_host(front: &conduit_core::CheckedFront) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/consumer"),
        boot_id: BootId::from("boot/consumer/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/consumer@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![conduit_core::capability_offer_from_parts! {
            semantic_contract: Default::default(),
            startup_parameters: front.startup_parameters().to_vec(),
            shorthand: front
                .shorthand()
                .map(|(input, output)| (input.clone(), output.clone())),
            capability_id: CapabilityId::from("consumer/pool-observe"),
            kind_id: kind_id("flow/pool-observe"),
            kind_contract_revision: KindIdentity::from("flow/pool-observe@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
                implementation_id: ImplementationId::from("consumer/pool-observe@1"),
                artifact_id: ArtifactId::from("artifact/consumer-pool-observe"),
            },
            inputs: front.inputs().to_vec(),
            outputs: front.outputs().to_vec(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        }],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from("test/planner"),
            limits: PlannerLimits {
                maximum_host_advertisements: 4,
                maximum_gears: 4,
                maximum_connections: 4,
                maximum_authority_grants: 4,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 8,
            },
        }],
    }
}

pub(super) fn authority() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/model-pool"),
        contract_id: AuthorityContractId::from(SHARED_POOL_ADMIT_AUTHORITY_CONTRACT),
        host_call_contract_id: HostCallContractId::from(SHARED_POOL_ADMIT_HOST_CALL_CONTRACT),
        subject_kind: kind_id(SHARED_POOL_AUTHORITY_SUBJECT_KIND),
        host_id: HostId::from("host/consumer"),
        boot_id: BootId::from("boot/consumer/1"),
        capability_id: CapabilityId::from("consumer/pool-observe"),
    }
}

pub(super) fn requirement() -> BTreeMap<SharedPoolId, SharedPoolPlanningRequirement> {
    BTreeMap::from([(
        SharedPoolId::from("model-service/workers"),
        SharedPoolPlanningRequirement {
            member_limits: PoolMemberLimits {
                queue_item_capacity: 1,
                queue_byte_capacity: 1,
                sign_item_capacity: 8,
                sign_byte_capacity: 1_024,
            },
            admission_authority: authority(),
            member_sessions_required: false,
        },
    )])
}

pub(super) fn build_plan(hosts: &[HostAdvertisement]) -> conduit_core::Plan {
    let plot = expanded();
    let placements = default_expanded_placements(&plot, hosts).unwrap();
    plan_expanded_canonical_with_shared_pools(
        &plot,
        hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &requirement(),
    )
    .unwrap()
}

pub(super) fn build_session_plan(
    hosts: &[HostAdvertisement],
    lines: &[conduit_core::LineOffer],
) -> Result<conduit_core::Plan, conduit_planner::PlannerError> {
    let plot = expanded();
    let placements = default_expanded_placements(&plot, hosts).unwrap();
    let remote_base = conduit_core::BaseImplementationId::from("conduit.base/test-line@1");
    let mut requirements = requirement();
    requirements
        .get_mut(&SharedPoolId::from("model-service/workers"))
        .unwrap()
        .member_sessions_required = true;
    plan_expanded_canonical_with_shared_pools(
        &plot,
        hosts,
        &placements,
        &[
            conduit_core::BaseImplementationId::from("conduit.base/local@1"),
            remote_base,
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: lines,
        },
        &requirements,
    )
}
