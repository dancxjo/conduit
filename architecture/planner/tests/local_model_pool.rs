use std::collections::BTreeMap;

use conduit_core::{
    kind_id, ArtifactId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BootId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PlannerCapabilityOffer, PlannerLimits, PlannerProfileId,
    PlanningRequestAuthority, PlayUnsatisfiedReason, PoolMemberLimits, PoolOperationId,
    PoolSelectionDisposition, PoolSelectionEvidence, SharedPoolId, SignId, PROTOCOL_VERSION,
    SHARED_POOL_ADMIT_AUTHORITY_CONTRACT, SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
    SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use conduit_kernel::{
    shared_pool::{
        admit_selected_pool_member, FixedSharedPool, LoweredObservationHealth,
        LoweredPoolObservation, LoweredPoolRealization, LoweredPoolResourceRequirement, MemberKey,
        MemberPlacement, PoolSelectionError, PoolSelectionPolicy,
    },
    NodeId,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_shared_pools, PlanningOptions,
    SharedPoolPlanningRequirement,
};

const SOURCE: &str = r#"
form model-worker (
    maximum-input-bytes: Count = 4096
    maximum-context-tokens: Count = 4096
    maximum-output-tokens: Count = 512
    temperature-milli: Count = 0
    prompt: Text > text: Text
) {
}

form consumer (
    workers: Pool
) {
    observe: flow/pool-observe(workers)
}

form model-service {
    pool workers: model-worker(size = 2)
    client: consumer(workers)
}
"#;

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
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
        .insert(KindDefinition {
            kind_id: kind_id("flow/pool-observe"),
            kind_contract_revision: KindContractRevision::from("flow/pool-observe@1"),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    (startup, profile)
}

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    expand_canonical_form(&checked, "model-service", &profile).unwrap()
}

fn consumer_host(front: &conduit_core::CheckedFace) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/consumer"),
        boot_id: BootId::from("boot/consumer/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/consumer@1"),
        resources: vec![],
        capabilities: vec![CapabilityOffer {
            startup_parameters: front.startup_parameters().to_vec(),
            shorthand: front
                .shorthand()
                .map(|(input, output)| (input.clone(), output.clone())),
            capability_id: CapabilityId::from("consumer/pool-observe"),
            kind_id: kind_id("flow/pool-observe"),
            kind_contract_revision: KindContractRevision::from("flow/pool-observe@1"),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
                implementation_id: ImplementationId::from("consumer/pool-observe@1"),
                artifact_id: ArtifactId::from("artifact/consumer-pool-observe"),
            },
            inputs: front.inputs().to_vec(),
            outputs: front.outputs().to_vec(),
            host_operations: vec![],
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
                maximum_line_offers: 0,
            },
        }],
    }
}

fn authority() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/model-pool"),
        contract_id: AuthorityContractId::from(SHARED_POOL_ADMIT_AUTHORITY_CONTRACT),
        host_operation_contract_id: HostOperationContractId::from(
            SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
        ),
        subject_kind: kind_id(SHARED_POOL_AUTHORITY_SUBJECT_KIND),
        host_id: HostId::from("host/consumer"),
        boot_id: BootId::from("boot/consumer/1"),
        capability_id: CapabilityId::from("consumer/pool-observe"),
    }
}

fn requirement() -> BTreeMap<SharedPoolId, SharedPoolPlanningRequirement> {
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
        },
    )])
}

fn build_plan(hosts: &[HostAdvertisement]) -> conduit_core::Plan {
    let form = expanded();
    let placements = default_expanded_placements(&form, hosts).unwrap();
    plan_expanded_canonical_with_shared_pools(
        &form,
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

fn key(value: u8) -> MemberKey {
    MemberKey([value; 32])
}

#[test]
fn two_generate_text_hosts_fallback_only_inside_the_immutable_plan_envelope() {
    let form = expanded();
    let observer = &form.gears[0];
    let consumer = consumer_host(&observer.checked_front());
    let fixtures = conduit_ai::generate_text_base_fixtures();
    let hosts = vec![
        consumer,
        fixtures[0].advertisement.clone(),
        fixtures[1].advertisement.clone(),
    ];
    assert_eq!(
        form.shared_pools[0].member_front,
        fixtures[0].advertisement.capabilities[0].checked_front()
    );
    let plan = build_plan(&hosts);
    assert!(conduit_core::verify_plan(&plan));
    let planned = &plan.fragments[0].shared_pools[0];
    assert_eq!(
        planned.member_front,
        fixtures[0].advertisement.capabilities[0].checked_front()
    );
    assert_eq!(planned.realization_envelope.len(), 2);
    assert_eq!(planned.realization_envelope[0].member_capacity, 1);
    assert_eq!(planned.realization_envelope[1].member_capacity, 1);

    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let lowered = &lowered.shared_pools[0];
    let envelope = lowered
        .realizations
        .iter()
        .map(|realization| LoweredPoolRealization {
            realization: realization.realization,
            placement: MemberPlacement {
                node: NodeId(realization.realization + 10),
                realization: realization.realization,
                play: 7,
            },
            boot: realization.realization + 20,
            offer_generation: realization.offer_generation.0,
            capability: realization.realization + 30,
            implementation: realization.realization + 40,
            artifact: realization.realization + 50,
            member_capacity: realization.member_capacity,
        })
        .collect::<Vec<_>>();
    let requirements = lowered
        .realizations
        .iter()
        .flat_map(|realization| {
            realization
                .resources
                .iter()
                .enumerate()
                .map(move |(index, binding)| LoweredPoolResourceRequirement {
                    realization: realization.realization,
                    resource: index as u16,
                    units: binding.units,
                })
        })
        .collect::<Vec<_>>();
    let observations = requirements
        .iter()
        .map(|required| {
            let realization = envelope[usize::from(required.realization)];
            LoweredPoolObservation {
                realization: required.realization,
                resource: required.resource,
                boot: realization.boot,
                offer_generation: realization.offer_generation,
                capability: realization.capability,
                implementation: realization.implementation,
                artifact: realization.artifact,
                health: LoweredObservationHealth::Ready,
                unreserved_units: required.units,
                utilized_units: 0,
                sign: 100 + required.realization * 10 + required.resource,
            }
        })
        .collect::<Vec<_>>();
    let mut pool = FixedSharedPool::<2, 32>::new(lowered.pool, 2, 7, 2).unwrap();
    let mut signs = [0; 8];
    let select = |pool: &mut FixedSharedPool<2, 32>,
                  key,
                  observations: &[LoweredPoolObservation],
                  signs: &mut [u16]| {
        admit_selected_pool_member(
            pool,
            key,
            7,
            &envelope,
            &requirements,
            observations,
            PoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
            signs,
        )
    };

    let first = select(&mut pool, key(1), &observations, &mut signs).unwrap();
    pool.trigger(first.member).unwrap();
    assert_eq!(first.member.placement.realization, 0);
    let second = select(&mut pool, key(2), &observations, &mut signs).unwrap();
    pool.trigger(second.member).unwrap();
    assert_eq!(second.member.placement.realization, 1);
    pool.request_release(second.member).unwrap();
    pool.complete_release(second.member).unwrap();

    pool.fail_member(first.member).unwrap();
    let mut lost = observations.clone();
    for observation in &mut lost {
        if observation.realization == 0 {
            observation.health = LoweredObservationHealth::Unavailable;
        }
    }
    assert_eq!(
        pool.population_for_realization(0),
        1,
        "failed work is not replayed"
    );
    let later = select(&mut pool, key(3), &lost, &mut signs).unwrap();
    pool.trigger(later.member).unwrap();
    assert_eq!(later.member.key, key(3));
    assert_eq!(later.member.placement.realization, 1);
    assert_eq!(
        select(&mut pool, key(4), &lost, &mut signs),
        Err(PoolSelectionError::NoCurrentRealization {
            examined_realizations: 2
        })
    );
    let exhausted = PoolSelectionEvidence {
        plan_id: plan.plan_id.clone(),
        pool_id: planned.pool_id.clone(),
        operation_id: PoolOperationId::from("model-request/4"),
        selected_realization: None,
        observation_sign_ids: vec![],
        disposition: PoolSelectionDisposition::EnvelopeExhausted,
        sign_id: SignId::from("sign/model-pool-exhausted/4"),
    };
    let events = exhausted
        .exhaustion_replan_events(
            &plan,
            HostId::from("host/consumer"),
            BootId::from("boot/consumer/1"),
            PlanningRequestAuthority::HostLocal,
            SignId::from("sign/model-pool-replan-request/4"),
        )
        .unwrap();
    assert!(matches!(
        &events[0],
        conduit_core::ControlLoopEvent::PlayBecameUnsatisfied {
            reason: PlayUnsatisfiedReason::NoAdmittedPoolRealizationReady,
            ..
        }
    ));
    assert!(matches!(
        &events[1],
        conduit_core::ControlLoopEvent::PlanningRequested { .. }
    ));

    let mut replacement_hosts = hosts.clone();
    replacement_hosts[1].boot_id = BootId::from("ai-small-local-boot/2");
    replacement_hosts[1].offer_generation = OfferGeneration(2);
    let replacement = build_plan(&replacement_hosts);
    assert_ne!(replacement.plan_id, plan.plan_id);
    assert_eq!(replacement.source_document_id, plan.source_document_id);
    assert_eq!(replacement.checked_form_id, plan.checked_form_id);
    assert_eq!(replacement.expanded_form_id, plan.expanded_form_id);
}
