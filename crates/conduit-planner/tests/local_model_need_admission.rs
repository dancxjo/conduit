use std::collections::BTreeMap;

use conduit_ai::{
    install_llm_semantic_catalog, LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy,
    LocalModelComputeNeed, LocalModelIdentity, LocalModelKindProfile, LocalModelLifecycleState,
    LocalModelLimits, LocalModelOffer, LOCAL_MODEL_COMPUTE_RESOURCE,
    LOCAL_MODEL_INFERENCE_SLOT_RESOURCE, LOCAL_MODEL_MEMORY_RESOURCE,
    LOCAL_MODEL_QUEUE_ITEM_RESOURCE, LOCAL_MODEL_QUEUE_KIB_RESOURCE,
};
use conduit_core::{
    compute_resource_offer, resource_offer, ArchitectureBaseId, ArchitectureBaseKind, BootId,
    ComputePoolContract, ComputeServiceGuarantee, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, ResourceAdmissionOwner, ResourceHealth, ResourceObservation, SignId,
};
use conduit_form::{ProfileCatalog, StartupCatalog};
use conduit_planner::{
    plan_with_hard_requirements, select_data_locality_candidate, CandidatePlacementDisposition,
    DataFlowObservation, LocalityCandidate, LocalityPlanningBasis, ObservationProvenance,
    PlacementChoice, PlacementChoices, RealizationWorkObservation,
};

fn provenance(id: &str) -> ObservationProvenance {
    ObservationProvenance {
        sign_id: SignId::from(id),
        source: "bounded local-model fixture".into(),
        observed_at_ms: 90,
        valid_until_ms: 110,
    }
}

fn checked_form() -> conduit_form::CheckedForm {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
    conduit_form::parse(
        "form model-placement {\n model: llm/generate(4096, 1, 1024, 4096, 0)\n}\n",
        &profiles,
    )
    .unwrap()
}

fn local_offer(model: &str, minimum: u32, preferred: u32, maximum: u32) -> LocalModelOffer {
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fixture-runtime".into(),
            runtime_version: "1".into(),
            runtime_build_identity: format!("runtime/{model}"),
            model_name: model.into(),
            model_content_identity: format!("sha256-{model}"),
            architecture: "transformer".into(),
            parameter_profile: "bounded".into(),
            quantization: "fixture".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds {
                maximum_input_bytes: 4_096,
                maximum_context_items: 1,
                maximum_output_bytes: 1_024,
                maximum_work_units: 4_096,
                maximum_history_items: 0,
            },
            model_bytes: 1,
            admitted_memory_mib: 8,
            compute: LocalModelComputeNeed {
                minimum_lanes: minimum,
                preferred_lanes: preferred,
                maximum_lanes: maximum,
                minimum_service_guarantee: ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 2,
            maximum_queue_bytes: 8_192,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::Generate],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}

fn host(id: &str, lanes: u32, need: (u32, u32, u32)) -> HostAdvertisement {
    let offer = local_offer(id, need.0, need.1, need.2);
    let construction = format!(
        "host {id} {{\n  schema = 1\n  target = {{architecture: \"x86_64\", machine: \"workstation\", os: \"linux\"}}\n  need = {{id: \"{id}/memory\", class: \"{LOCAL_MODEL_MEMORY_RESOURCE}\", slots: 8, bytes: 1}}\n  need = {{id: \"{id}/compute\", class: \"{LOCAL_MODEL_COMPUTE_RESOURCE}\", slots: {lanes}, bytes: 1}}\n  need = {{id: \"{id}/slot\", class: \"{LOCAL_MODEL_INFERENCE_SLOT_RESOURCE}\", slots: 1, bytes: 1}}\n  need = {{id: \"{id}/queue-items\", class: \"{LOCAL_MODEL_QUEUE_ITEM_RESOURCE}\", slots: 2, bytes: 1}}\n  need = {{id: \"{id}/queue-kib\", class: \"{LOCAL_MODEL_QUEUE_KIB_RESOURCE}\", slots: 8, bytes: 1}}\n  limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}\n}}\n"
    );
    let checked = conduit_host_fabrication::check_host_configuration(
        conduit_host_fabrication::parse_host_configuration_conduit(&construction).unwrap(),
        &conduit_host_fabrication::FabricationCatalog::canonical(),
    )
    .unwrap();
    let mut resources = checked
        .configuration()
        .resources
        .iter()
        .map(|budget| {
            if budget.class == LOCAL_MODEL_COMPUTE_RESOURCE {
                compute_resource_offer(
                    &budget.id,
                    &budget.class,
                    budget.slots,
                    ComputePoolContract {
                        service_guarantee: ComputeServiceGuarantee::Shared,
                        architecture_base_id: ArchitectureBaseId::from(format!(
                            "{id}/hosted-compute"
                        )),
                        architecture_base_kind: ArchitectureBaseKind::HostedOs,
                        topology_groups: vec![],
                    },
                )
            } else {
                resource_offer(&budget.id, &budget.class, budget.slots)
            }
        })
        .collect::<Vec<_>>();
    resources.sort();
    HostAdvertisement {
        protocol_version: 1,
        host_id: HostId::from(format!("host/{id}")),
        boot_id: BootId::from(format!("boot/{id}/1")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduit.host/local-model-fixture@1"),
        resources,
        capabilities: offer.capability_offers().unwrap(),
        planner_capabilities: vec![],
    }
}

fn candidate(
    form: &conduit_form::CheckedForm,
    host: &HostAdvertisement,
    id: &str,
) -> LocalityCandidate {
    LocalityCandidate {
        candidate_id: id.into(),
        placements: PlacementChoices {
            by_gear: BTreeMap::from([(
                form.gears[0].gear_id.clone(),
                PlacementChoice {
                    host_id: host.host_id.clone(),
                    capability_id: host.capabilities[0].capability_id.clone(),
                },
            )]),
        },
        lines: BTreeMap::new(),
    }
}

fn observations(hosts: &[HostAdvertisement]) -> Vec<ResourceObservation> {
    hosts
        .iter()
        .flat_map(|host| {
            host.resources
                .iter()
                .enumerate()
                .map(move |(index, pool)| ResourceObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: ResourceHealth::Ready,
                    unreserved_units: pool.capacity_units,
                    utilized_units: 0,
                    sign_id: SignId::from(format!("sign/{}/{index}", host.host_id.as_str())),
                })
        })
        .collect()
}

fn basis(
    form: &conduit_form::CheckedForm,
    hosts: &[HostAdvertisement],
    resources: Vec<ResourceObservation>,
    costs: [u64; 2],
) -> LocalityPlanningBasis {
    LocalityPlanningBasis {
        now_ms: 100,
        horizon_seconds: 1,
        remote_bytes_per_second_ceiling: None,
        data_flow: DataFlowObservation {
            source_gear_id: form.gears[0].gear_id.clone(),
            items_per_second: 1,
            bytes_per_item: 1,
            provenance: provenance("sign/request-work"),
        },
        reductions: vec![],
        realization_work: hosts
            .iter()
            .zip(costs)
            .enumerate()
            .map(|(index, (host, work_units))| RealizationWorkObservation {
                gear_id: form.gears[0].gear_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                capability_id: host.capabilities[0].capability_id.clone(),
                work_units,
                provenance: provenance(&format!("sign/model-cost/{index}")),
            })
            .collect(),
        transports: vec![],
        local_cords: vec![],
        resources,
    }
}

fn set_available(
    observations: &mut [ResourceObservation],
    host: &HostAdvertisement,
    class: &str,
    units: u32,
) {
    let observation = observations
        .iter_mut()
        .find(|item| item.host_id == host.host_id && item.class_id.as_str() == class)
        .unwrap();
    observation.unreserved_units = units;
}

#[test]
fn cross_host_local_models_separate_hard_admission_from_observed_cost_selection() {
    let form = checked_form();
    let form_identity = form.checked_form_id.clone();
    let hosts = vec![host("compact", 6, (2, 4, 6)), host("wide", 16, (4, 8, 12))];
    let candidates = [
        candidate(&form, &hosts[0], "compact"),
        candidate(&form, &hosts[1], "wide"),
    ];

    let first = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, observations(&hosts), [20, 80]),
        &[],
    )
    .unwrap();
    assert_eq!(first.selected.candidate_id, "compact");
    assert!(matches!(
        first.considered[1].disposition,
        CandidatePlacementDisposition::Admitted
    ));
    assert!(first
        .considered
        .iter()
        .all(|item| !item.supporting_sign_ids.is_empty()));

    let second = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, observations(&hosts), [90, 10]),
        &[],
    )
    .unwrap();
    assert_eq!(second.selected.candidate_id, "wide");
    assert_eq!(second.checked_form_id, form_identity);
    assert_eq!(first.checked_form_id, second.checked_form_id);
}

#[test]
fn compute_slot_and_provider_pressure_are_hard_refusals_while_high_cost_is_not() {
    let form = checked_form();
    let hosts = vec![host("compact", 6, (2, 4, 6)), host("wide", 16, (4, 8, 12))];
    let candidates = [
        candidate(&form, &hosts[0], "compact"),
        candidate(&form, &hosts[1], "wide"),
    ];

    let mut compute_full = observations(&hosts);
    set_available(
        &mut compute_full,
        &hosts[0],
        LOCAL_MODEL_COMPUTE_RESOURCE,
        1,
    );
    let selected = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, compute_full, [1, 100]),
        &[],
    )
    .unwrap();
    assert_eq!(selected.selected.candidate_id, "wide");
    assert!(matches!(
        selected.considered[0].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));

    let mut slot_full = observations(&hosts);
    set_available(
        &mut slot_full,
        &hosts[0],
        LOCAL_MODEL_INFERENCE_SLOT_RESOURCE,
        0,
    );
    let selected = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, slot_full, [1, 100]),
        &[],
    )
    .unwrap();
    assert!(matches!(
        selected.considered[0].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));

    let mut provider_lost = observations(&hosts);
    let slot = provider_lost
        .iter_mut()
        .find(|item| {
            item.host_id == hosts[0].host_id
                && item.class_id.as_str() == LOCAL_MODEL_INFERENCE_SLOT_RESOURCE
        })
        .unwrap();
    slot.health = ResourceHealth::Unavailable;
    slot.unreserved_units = 0;
    let selected = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, provider_lost, [1, 100]),
        &[],
    )
    .unwrap();
    assert!(matches!(
        selected.considered[0].disposition,
        CandidatePlacementDisposition::Rejected(_)
    ));
    assert!(selected.explain().contains("rejected"));
}

#[test]
fn selected_need_becomes_exact_plan_binding_then_owner_admission() {
    let form = checked_form();
    let host = host("compact", 6, (2, 4, 6));
    let placements = candidate(&form, &host, "compact").placements;
    let plan = plan_with_hard_requirements(
        &form,
        std::slice::from_ref(&host),
        &placements,
        &[],
        &BTreeMap::new(),
    )
    .unwrap();
    let placement = &plan.fragments[0].placements[0];
    let compute = placement
        .resources
        .iter()
        .find(|binding| binding.class_id.as_str() == LOCAL_MODEL_COMPUTE_RESOURCE)
        .unwrap();
    assert_eq!(compute.units, 4);

    let current = observations(std::slice::from_ref(&host));
    let mut owner = ResourceAdmissionOwner::new(host);
    let admission = owner
        .admit_planned_placement(plan.plan_id.clone(), placement, &current)
        .unwrap();
    assert_eq!(admission.plan_id, plan.plan_id);
    assert_eq!(admission.items.len(), placement.resources.len());
    assert_eq!(
        admission.observation_sign_ids.len(),
        placement.resources.len()
    );
}
