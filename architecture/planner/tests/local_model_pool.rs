use std::collections::BTreeMap;

use conduit_core::{
    kind_id, ArtifactId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BootId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PlannerCapabilityOffer, PlannerLimits, PlannerProfileId,
    PlanningRequestAuthority, PlayUnsatisfiedReason, PoolMemberLimits, PoolOperationId,
    PoolRealizationHealth, PoolRealizationObservation, PoolSelectionDisposition,
    PoolSelectionEvidence, ResourceHealth, ResourceObservation, SharedPoolId, SignId,
    PROTOCOL_VERSION, SHARED_POOL_ADMIT_AUTHORITY_CONTRACT,
    SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT, SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature,
};
use conduit_kernel::{
    shared_pool::{
        admit_selected_pool_member, FixedSharedPool, LoweredObservationHealth,
        LoweredPoolObservation, MemberKey, PoolSelectionError, PoolSelectionPolicy,
    },
    NodeId,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_shared_pools, PlanningOptions,
    SharedPoolPlanningRequirement,
};
use serde::Serialize;

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

fn with_capacity(mut host: HostAdvertisement, capacity: u16) -> HostAdvertisement {
    host.capabilities[0].limits.max_active_instances = capacity;
    for resource in &mut host.resources {
        resource.capacity_units = resource.capacity_units.max(64);
    }
    host
}

#[derive(Serialize)]
struct SelectionReceipt {
    operation_id: &'static str,
    disposition: &'static str,
    realization: Option<u16>,
    observation_signs: Vec<String>,
    planning_requested: bool,
}

#[derive(Serialize)]
struct DeterministicReceipt<'a> {
    schema: &'static str,
    proof_class: &'static str,
    physical_evidence: bool,
    body_id: &'static str,
    wake_id: &'static str,
    source_document_id: &'a str,
    checked_form_id: &'a str,
    expanded_form_id: &'a str,
    plan_id: &'a str,
    play_id: &'a str,
    pool_id: &'a str,
    realization_envelope: &'a [conduit_core::PoolRealizationEnvelope],
    selections: Vec<SelectionReceipt>,
    unsealed_host_refused: bool,
    replacement_plan_id: &'a str,
    replacement_play_id: &'a str,
    replacement_preserved_body_and_wake: bool,
}

fn retain_receipt(receipt: &DeterministicReceipt<'_>) {
    let Some(path) = std::env::var_os("CONDUIT_LOCAL_MODEL_POOL_RECEIPT_PATH") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(receipt).unwrap();
    std::fs::write(&temporary, bytes).unwrap();
    std::fs::rename(temporary, path).unwrap();
}

fn current_observations(pool: &conduit_core::PlannedSharedPool) -> Vec<PoolRealizationObservation> {
    pool.realization_envelope
        .iter()
        .enumerate()
        .map(
            |(realization_index, realization)| PoolRealizationObservation {
                host_id: realization.host_id.clone(),
                boot_id: realization.boot_id.clone(),
                offer_generation: realization.offer_generation,
                capability_id: realization.capability_id.clone(),
                implementation_id: realization.implementation_id.clone(),
                artifact_id: realization.artifact_id.clone(),
                health: PoolRealizationHealth::Ready,
                sign_id: SignId::from(format!("sign/provider/{realization_index}")),
                resources: realization
                    .resources
                    .iter()
                    .enumerate()
                    .map(|(resource_index, binding)| ResourceObservation {
                        host_id: realization.host_id.clone(),
                        boot_id: realization.boot_id.clone(),
                        offer_generation: realization.offer_generation,
                        pool_id: binding.pool_id.clone(),
                        class_id: binding.class_id.clone(),
                        health: ResourceHealth::Ready,
                        unreserved_units: binding.units,
                        utilized_units: 0,
                        sign_id: SignId::from(format!(
                            "sign/resource/{realization_index}/{resource_index}"
                        )),
                    })
                    .collect(),
            },
        )
        .collect()
}

fn selected_signs(
    selected: &conduit_kernel::shared_pool::SelectedPoolMember,
    signs: &[u16],
    identities: &[SignId],
) -> Vec<String> {
    signs[..usize::from(selected.observation_sign_count)]
        .iter()
        .map(|index| identities[usize::from(*index)].as_str().to_owned())
        .collect()
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
    let current = current_observations(planned);
    let facts = lowered
        .lower_selection_facts(&current, NodeId(10), 7)
        .unwrap();
    let mut stale_current = current.clone();
    stale_current[0].boot_id = BootId::from("stale-boot");
    let stale_facts = lowered
        .lower_selection_facts(&stale_current, NodeId(10), 7)
        .unwrap();
    assert!(!stale_facts
        .observations
        .iter()
        .any(|observation| observation.realization == 0));
    let mut duplicate_current = current.clone();
    duplicate_current.push(current[0].clone());
    assert_eq!(
        lowered.lower_selection_facts(&duplicate_current, NodeId(10), 7),
        Err(
            conduit_plan_lowering::lowering::PoolObservationLoweringError::DuplicateCurrentObservation
        )
    );
    let envelope = facts.envelope;
    let requirements = facts.requirements;
    let observations = facts.observations;
    let observation_sign_ids = facts.observation_sign_ids;
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
    let first_signs = selected_signs(&first, &signs, &observation_sign_ids);
    pool.trigger(first.member).unwrap();
    assert_eq!(first.member.placement.realization, 0);
    let second = select(&mut pool, key(2), &observations, &mut signs).unwrap();
    let second_signs = selected_signs(&second, &signs, &observation_sign_ids);
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
    let later_signs = selected_signs(&later, &signs, &observation_sign_ids);
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

    let a_only_hosts = vec![
        hosts[0].clone(),
        with_capacity(fixtures[0].advertisement.clone(), 2),
    ];
    let a_only = build_plan(&a_only_hosts);
    assert_eq!(
        a_only.fragments[0].shared_pools[0]
            .realization_envelope
            .len(),
        1
    );
    assert_eq!(
        a_only.fragments[0].shared_pools[0].realization_envelope[0].member_capacity,
        2
    );
    assert!(!a_only.fragments[0].shared_pools[0]
        .realization_envelope
        .iter()
        .any(|realization| realization.host_id == hosts[2].host_id));

    let replacement_hosts = vec![
        hosts[0].clone(),
        with_capacity(fixtures[1].advertisement.clone(), 2),
    ];
    let replacement = build_plan(&replacement_hosts);
    assert_ne!(replacement.plan_id, plan.plan_id);
    assert_ne!(replacement.plan_id, a_only.plan_id);
    assert_eq!(replacement.source_document_id, a_only.source_document_id);
    assert_eq!(replacement.checked_form_id, a_only.checked_form_id);
    assert_eq!(replacement.expanded_form_id, a_only.expanded_form_id);
    let a_only_pool = &a_only.fragments[0].shared_pools[0];
    let b_replacement = &replacement.fragments[0].shared_pools[0].realization_envelope[0];
    assert!(!a_only_pool.permits_realization(b_replacement, &a_only_pool.member_front));
    let play = conduit_core::bind_active_play(
        &plan.plan_id,
        &HostId::from("host/consumer"),
        &BootId::from("boot/consumer/1"),
        1,
    );
    let replacement_play = conduit_core::bind_active_play(
        &replacement.plan_id,
        &HostId::from("host/consumer"),
        &BootId::from("boot/consumer/1"),
        2,
    );
    retain_receipt(&DeterministicReceipt {
        schema: "conduit.local-model-pool-proof/v1",
        proof_class: "deterministic-hosted-integration",
        physical_evidence: false,
        body_id: "body/local-model-pool-fixture",
        wake_id: "wake/local-model-pool-fixture/1",
        source_document_id: plan.source_document_id.as_str(),
        checked_form_id: plan.checked_form_id.as_str(),
        expanded_form_id: plan.expanded_form_id.as_str(),
        plan_id: plan.plan_id.as_str(),
        play_id: play.active_play_id.as_str(),
        pool_id: planned.pool_id.as_str(),
        realization_envelope: &planned.realization_envelope,
        selections: vec![
            SelectionReceipt {
                operation_id: "model-request/1",
                disposition: "selected",
                realization: Some(first.member.placement.realization),
                observation_signs: first_signs,
                planning_requested: false,
            },
            SelectionReceipt {
                operation_id: "model-request/2",
                disposition: "selected-after-capacity-refusal",
                realization: Some(second.member.placement.realization),
                observation_signs: second_signs,
                planning_requested: false,
            },
            SelectionReceipt {
                operation_id: "model-request/1",
                disposition: "provider-lost-no-replay",
                realization: Some(first.member.placement.realization),
                observation_signs: vec![],
                planning_requested: false,
            },
            SelectionReceipt {
                operation_id: "model-request/3",
                disposition: "selected-sealed-fallback",
                realization: Some(later.member.placement.realization),
                observation_signs: later_signs,
                planning_requested: false,
            },
            SelectionReceipt {
                operation_id: "model-request/4",
                disposition: "envelope-exhausted",
                realization: None,
                observation_signs: vec![],
                planning_requested: true,
            },
        ],
        unsealed_host_refused: true,
        replacement_plan_id: replacement.plan_id.as_str(),
        replacement_play_id: replacement_play.active_play_id.as_str(),
        replacement_preserved_body_and_wake: true,
    });
}
