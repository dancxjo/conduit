mod common;

use std::collections::BTreeMap;

use conduit_ai::{
    install_llm_semantic_catalog, LocalModelKindProfile, LOCAL_MODEL_COMPUTE_RESOURCE,
    LOCAL_MODEL_INFERENCE_SLOT_RESOURCE,
};
use conduit_core::{
    HostAdvertisement, ResourceAdmissionOwner, ResourceHealth, ResourceObservation, SignId,
};
use conduit_form::{ProfileCatalog, StartupCatalog};
use conduit_planner::{
    plan_with_hard_requirements, select_data_locality_candidate, CandidatePlacementDisposition,
    DataFlowObservation, LocalityCandidate, LocalityPlanningBasis, ObservationProvenance,
    PlacementChoice, PlacementChoices, RealizationWorkObservation,
};

use common::local_model::{dual_local_model_providers, local_model_provider};

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
    let fixtures = dual_local_model_providers();
    let hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
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

    let compact_plan = plan_with_hard_requirements(
        &form,
        &hosts,
        &first.selected.placements,
        &[],
        &BTreeMap::new(),
    )
    .unwrap();
    let wide_plan = plan_with_hard_requirements(
        &form,
        &hosts,
        &second.selected.placements,
        &[],
        &BTreeMap::new(),
    )
    .unwrap();
    let compact = &compact_plan.fragments[0].placements[0];
    let wide = &wide_plan.fragments[0].placements[0];
    assert_eq!(compact_plan.checked_form_id, wide_plan.checked_form_id);
    assert_ne!(compact_plan.plan_id, wide_plan.plan_id);
    assert_eq!(compact.kind_id, wide.kind_id);
    assert_ne!(compact.host_id, wide.host_id);
    assert_ne!(compact.boot_id, wide.boot_id);
    assert_ne!(compact.artifact_id, wide.artifact_id);
    assert_ne!(fixtures[0].provider.identity, fixtures[1].provider.identity);
    assert_eq!(compact.host_id, fixtures[0].advertisement.host_id);
    assert_eq!(compact.boot_id, fixtures[0].advertisement.boot_id);
    assert_eq!(wide.host_id, fixtures[1].advertisement.host_id);
    assert_eq!(wide.boot_id, fixtures[1].advertisement.boot_id);
    for (planned, fixture) in [compact, wide].into_iter().zip(&fixtures) {
        let offered = &fixture.advertisement.capabilities[0];
        assert_eq!(planned.capability_id, offered.capability_id);
        assert_eq!(
            planned.implementation_id,
            offered.implementation.implementation_id
        );
        assert_eq!(planned.artifact_id, offered.implementation.artifact_id);
        assert_eq!(planned.kind_id, offered.kind_id);
        assert_eq!(form.gears[0].checked_front(), offered.checked_front());
    }
}

#[test]
fn non_equivalent_local_model_profile_is_not_a_generate_substitute() {
    let form = checked_form();
    let generate = dual_local_model_providers()[0].clone();
    let classify = local_model_provider(
        "classifier",
        6,
        (2, 4, 6),
        LocalModelKindProfile::ClassifyFiniteLabels,
    );
    let hosts = vec![generate.advertisement, classify.advertisement];
    let candidates = [
        candidate(&form, &hosts[0], "generate"),
        candidate(&form, &hosts[1], "classify"),
    ];

    let selection = select_data_locality_candidate(
        &form,
        &hosts,
        &candidates,
        &basis(&form, &hosts, observations(&hosts), [20, 1]),
        &[],
    )
    .unwrap();
    assert_eq!(selection.selected.candidate_id, "generate");
    assert!(matches!(
        &selection.considered[1].disposition,
        CandidatePlacementDisposition::Rejected(reason)
            if reason.contains("does not offer the checked front")
    ));
    assert!(matches!(
        plan_with_hard_requirements(
            &form,
            &hosts,
            &candidates[1].placements,
            &[],
            &BTreeMap::new(),
        ),
        Err(conduit_planner::PlannerError::WrongSemanticKind(_))
    ));
}

#[test]
fn compute_slot_and_provider_pressure_are_hard_refusals_while_high_cost_is_not() {
    let form = checked_form();
    let hosts = dual_local_model_providers()
        .into_iter()
        .map(|fixture| fixture.advertisement)
        .collect::<Vec<_>>();
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
    let host = dual_local_model_providers()[0].advertisement.clone();
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
