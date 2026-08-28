use conduit_core::{
    kind_id, ArtifactId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BootId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PlannerCapabilityOffer, PlannerLimits, PlannerProfileId, PoolMemberLimits,
    SharedPoolId, PROTOCOL_VERSION, SHARED_POOL_ADMIT_AUTHORITY_CONTRACT,
    SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT, SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_shared_pools, PlanningOptions,
    SharedPoolPlanningRequirement,
};
use std::collections::{BTreeMap, BTreeSet};

const SOURCE: &str = "form chat/peer (\n recv: ChatMessage...| > send: ChatMessage...|\n) {\n}\n\nform consumer (\n members: Pool\n) {\n use: flow/pool-observe(members)\n}\n\nform room {\n pool peers: chat/peer(size = 2)\n left: consumer(peers)\n right: consumer(peers)\n}\n";

fn peer_face() -> conduit_core::CheckedFace {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &startup_with_observe()).unwrap();
    checked
        .forms
        .iter()
        .find(|form| form.name == "chat/peer")
        .unwrap()
        .checked_face()
}

fn startup_with_observe() -> StartupCatalog {
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: "flow/pool-observe".into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "members".into(),
                value_type: "Pool".into(),
                default: None,
            }],
        })
        .unwrap();
    startup
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let startup = startup_with_observe();
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindDefinition {
            kind_id: kind_id("flow/pool-observe"),
            kind_contract_revision: KindContractRevision::from("flow/pool-observe@1"),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    expand_canonical_form(&checked, "room", &profile).unwrap()
}

fn offer_from_face(
    capability: &str,
    kind: &str,
    face: &conduit_core::CheckedFace,
    maximum: u16,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: face.startup_parameters().to_vec(),
        shorthand: face
            .shorthand()
            .map(|(input, output)| (input.clone(), output.clone())),
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@9")),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/hosted@1"),
            implementation_id: ImplementationId::from(format!("implementation/{capability}")),
            artifact_id: ArtifactId::from(format!("artifact/{capability}")),
        },
        inputs: face.inputs().to_vec(),
        outputs: face.outputs().to_vec(),
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: maximum,
            max_queue_items: 4,
            max_queue_bytes: 1_024,
        },
    }
}

fn host(form: &conduit_form::ExpandedCanonicalForm) -> HostAdvertisement {
    let observe = form.gears.first().expect("expanded room has observers");
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser"),
        boot_id: BootId::from("browser-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-profile"),
        resources: vec![],
        capabilities: vec![
            offer_from_face(
                "browser/pool-observe",
                "flow/pool-observe",
                &observe.checked_face(),
                2,
            ),
            offer_from_face(
                "browser/renamed-peer",
                "renamed/browser-peer",
                &peer_face(),
                2,
            ),
        ],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from("test/planner"),
            limits: PlannerLimits {
                maximum_host_advertisements: 4,
                maximum_gears: 8,
                maximum_connections: 8,
                maximum_authority_grants: 8,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 0,
            },
        }],
    }
}

fn authority() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/room-admission"),
        contract_id: AuthorityContractId::from(SHARED_POOL_ADMIT_AUTHORITY_CONTRACT),
        host_operation_contract_id: HostOperationContractId::from(
            SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
        ),
        subject_kind: kind_id(SHARED_POOL_AUTHORITY_SUBJECT_KIND),
        host_id: HostId::from("browser"),
        boot_id: BootId::from("browser-boot"),
        capability_id: CapabilityId::from("browser/pool-observe"),
    }
}

fn requirements() -> BTreeMap<SharedPoolId, SharedPoolPlanningRequirement> {
    BTreeMap::from([(
        SharedPoolId::from("room/peers"),
        SharedPoolPlanningRequirement {
            member_limits: PoolMemberLimits {
                queue_item_capacity: 4,
                queue_byte_capacity: 1_024,
                sign_item_capacity: 16,
                sign_byte_capacity: 2_048,
            },
            admission_authority: authority(),
        },
    )])
}

#[test]
fn canonical_pool_plans_equal_face_members_and_exact_consumers_envelope_and_authority() {
    let form = expanded();
    let host = host(&form);
    let placements = default_expanded_placements(&form, std::slice::from_ref(&host)).unwrap();
    let plan = plan_expanded_canonical_with_shared_pools(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 1_024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &requirements(),
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    let pool = &plan.fragments[0].shared_pools[0];
    assert_eq!(pool.maximum_members, 2);
    assert_eq!(pool.admission_authority.as_str(), "grant/room-admission");
    assert_eq!(pool.realization_envelope[0].member_capacity, 2);
    assert_eq!(
        pool.realization_envelope[0].capability_id.as_str(),
        "browser/renamed-peer"
    );
    assert_eq!(pool.consumers.len(), 2);
    assert_eq!(
        pool.consumers
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .len(),
        2
    );
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    assert_eq!(lowered.shared_pools.len(), 1);
    assert_eq!(lowered.shared_pools[0].pool.0, 0);
    assert_eq!(lowered.shared_pools[0].maximum_members, 2);
    assert_eq!(lowered.shared_pools[0].local_consumers.len(), 2);
    assert_eq!(lowered.shared_pools[0].realizations[0].member_capacity, 2);
}

#[test]
fn pool_planning_fails_when_face_capacity_or_authority_scope_is_not_exact() {
    let form = expanded();
    let mut host = host(&form);
    host.capabilities[1].limits.max_active_instances = 1;
    let placements = default_expanded_placements(&form, std::slice::from_ref(&host)).unwrap();
    let options = PlanningOptions {
        connection_bases: &BTreeMap::new(),
        line_candidates: &BTreeMap::new(),
        connection_item_capacity: 4,
        connection_byte_capacity: 1_024,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    assert!(plan_expanded_canonical_with_shared_pools(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1"
        )],
        options,
        &requirements(),
    )
    .is_err());

    host.capabilities[1].limits.max_active_instances = 2;
    let mut wrong = requirements();
    wrong
        .get_mut(&SharedPoolId::from("room/peers"))
        .unwrap()
        .admission_authority
        .contract_id = AuthorityContractId::from("wrong/authority");
    assert!(plan_expanded_canonical_with_shared_pools(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1"
        )],
        options,
        &wrong,
    )
    .is_err());
}
