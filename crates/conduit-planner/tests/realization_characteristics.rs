use conduit_ai::{
    generate_text_base_fixtures, generate_text_realization_advertisements,
    install_generate_text_catalog, DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC,
    METERED_COST_CHARACTERISTIC,
};
use conduit_core::{RealizationCharacteristicId, ResourceHealth, ResourceObservation, SignId};
use conduit_planner::{
    plan_selected_realizations_with_characteristics,
    replan_selected_realizations_with_characteristics,
    select_realization_with_characteristics_and_signs, HardRealizationRequirements,
    PlanningOptions, RealizationDecisionDisposition, RealizationPolicy, RealizationPreference,
    RealizationRejection, RealizationReplanOutcome,
};
use std::collections::BTreeMap;

fn form() -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    install_generate_text_catalog(&mut startup, &mut profile).expect("catalog installs");
    conduit_form::parse(
        "form 0\n\nanswer {\n    generate: ai/generate-text\n}\n",
        &profile,
    )
    .expect("form checks")
}

fn observations(hosts: &[conduit_core::HostAdvertisement]) -> Vec<ResourceObservation> {
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
                    sign_id: SignId::from(format!("{}-observation-{index}", host.host_id.as_str())),
                })
        })
        .collect()
}

#[test]
fn context_and_privacy_hard_requirements_select_only_large_local() {
    let form = form();
    let fixtures = generate_text_base_fixtures();
    let hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let gear = &form.gears[0];
    let requirements = HardRealizationRequirements {
        minimum_characteristic_counts: BTreeMap::from([(
            RealizationCharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC),
            24_000,
        )]),
        required_characteristic_flags: BTreeMap::from([(
            RealizationCharacteristicId::from(DATA_EGRESS_CHARACTERISTIC),
            false,
        )]),
        ..HardRealizationRequirements::default()
    };
    let requirement_map = BTreeMap::from([(gear.gear_id.clone(), requirements)]);
    let plan = plan_selected_realizations_with_characteristics(
        &form,
        &hosts,
        &[],
        &requirement_map,
        &advertisements,
        &observations(&hosts),
        &BTreeMap::new(),
    )
    .expect("hard requirements select the large local fixture");
    let selected = &plan.fragments[0].placements[0];
    assert_eq!(selected.host_id.as_str(), "ai-large-local");
    let mut expected = advertisements[1].characteristics.clone();
    expected.sort();
    assert_eq!(selected.realization_characteristics, expected);

    let mut changed_advertisements = advertisements.clone();
    changed_advertisements[1].characteristics[0].value =
        conduit_core::RealizationCharacteristicValue::Count(32_769);
    let changed = plan_selected_realizations_with_characteristics(
        &form,
        &hosts,
        &[],
        &requirement_map,
        &changed_advertisements,
        &observations(&hosts),
        &BTreeMap::new(),
    )
    .expect("changed stable fact replans");
    assert_ne!(plan.plan_id, changed.plan_id);
}

#[test]
fn bounded_decision_sign_explains_rejections_and_exact_selection() {
    let form = form();
    let fixtures = generate_text_base_fixtures();
    let hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let requirements = HardRealizationRequirements {
        minimum_characteristic_counts: BTreeMap::from([(
            RealizationCharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC),
            24_000,
        )]),
        required_characteristic_flags: BTreeMap::from([(
            RealizationCharacteristicId::from(DATA_EGRESS_CHARACTERISTIC),
            false,
        )]),
        ..HardRealizationRequirements::default()
    };

    let selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &requirements,
        &observations(&hosts),
        &RealizationPolicy::default(),
    )
    .expect("decision signs accompanies selection");
    assert_eq!(selection.choice.host_id.as_str(), "ai-large-local");
    assert_eq!(selection.signs.len(), 3);
    let small = selection
        .signs
        .iter()
        .find(|record| record.host_id.as_str() == "ai-small-local")
        .expect("small candidate is recorded");
    assert_eq!(
        small.disposition,
        RealizationDecisionDisposition::Rejected(RealizationRejection::MinimumCharacteristicCount(
            RealizationCharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC)
        ))
    );
    let large = selection
        .signs
        .iter()
        .find(|record| record.host_id.as_str() == "ai-large-local")
        .expect("large candidate is recorded");
    assert_eq!(large.disposition, RealizationDecisionDisposition::Selected);
    assert_eq!(
        large.implementation_id,
        fixtures[1].advertisement.capabilities[0]
            .implementation
            .implementation_id
    );
    assert_eq!(
        large.artifact_id,
        fixtures[1].advertisement.capabilities[0]
            .implementation
            .artifact_id
    );
    let remote = selection
        .signs
        .iter()
        .find(|record| record.host_id.as_str() == "ai-remote-base")
        .expect("remote candidate is recorded");
    assert_eq!(
        remote.disposition,
        RealizationDecisionDisposition::Rejected(RealizationRejection::RequiredCharacteristicFlag(
            RealizationCharacteristicId::from(DATA_EGRESS_CHARACTERISTIC)
        ))
    );
}

#[test]
fn decision_sign_fails_before_exceeding_its_candidate_bound() {
    let form = form();
    let fixture = generate_text_base_fixtures()[0].clone();
    let mut hosts = Vec::new();
    let mut advertisements = Vec::new();
    for index in 0..=conduit_planner::MAXIMUM_REALIZATION_DECISION_RECORDS {
        let mut host = fixture.advertisement.clone();
        host.host_id = conduit_core::HostId::from(format!("bounded-host-{index:03}"));
        host.boot_id = conduit_core::BootId::from(format!("bounded-boot-{index:03}"));
        host.capabilities[0].capability_id =
            conduit_core::CapabilityId::from(format!("bounded-capability-{index:03}"));
        let mut facts = conduit_ai::generate_text_realization_advertisements(&[
            conduit_ai::GenerateTextBaseFixture {
                advertisement: host.clone(),
                facts: fixture.facts.clone(),
            },
        ]);
        advertisements.push(facts.pop().expect("one realization fact"));
        hosts.push(host);
    }
    let error = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&hosts),
        &RealizationPolicy::default(),
    )
    .expect_err("candidate signs cannot grow above its fixed planning bound");
    assert!(matches!(
        error,
        conduit_planner::PlannerError::PlannerLimitExceeded(_)
    ));
}

#[test]
fn explicit_policy_can_prefer_remote_among_hard_admissible_candidates() {
    let form = form();
    let fixtures = generate_text_base_fixtures();
    let hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let selection = select_realization_with_characteristics_and_signs(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&hosts),
        &RealizationPolicy {
            preferences: vec![RealizationPreference::PreferCharacteristicFlag {
                characteristic_id: RealizationCharacteristicId::from(METERED_COST_CHARACTERISTIC),
                value: true,
            }],
        },
    )
    .expect("explicit policy selects a metered candidate");
    assert_eq!(selection.choice.host_id.as_str(), "ai-remote-base");
    assert_eq!(
        selection
            .signs
            .iter()
            .filter(|record| record.disposition == RealizationDecisionDisposition::Admitted)
            .count(),
        2
    );
}

#[test]
fn refreshed_observations_produce_a_new_plan_without_mutating_the_old_plan() {
    let form = form();
    let fixtures = generate_text_base_fixtures();
    let hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let gear_id = form.gears[0].gear_id.clone();
    let requirements = BTreeMap::from([(
        gear_id.clone(),
        HardRealizationRequirements {
            minimum_characteristic_counts: BTreeMap::from([(
                RealizationCharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC),
                24_000,
            )]),
            ..HardRealizationRequirements::default()
        },
    )]);
    let policies = BTreeMap::from([(
        gear_id,
        RealizationPolicy {
            preferences: vec![RealizationPreference::PreferCharacteristicFlag {
                characteristic_id: RealizationCharacteristicId::from(METERED_COST_CHARACTERISTIC),
                value: false,
            }],
        },
    )]);
    let initial_observations = observations(&hosts);
    let plan_a = plan_selected_realizations_with_characteristics(
        &form,
        &hosts,
        &[],
        &requirements,
        &advertisements,
        &initial_observations,
        &policies,
    )
    .expect("initial observations select large local");
    assert_eq!(
        plan_a.fragments[0].placements[0].implementation_id.as_str(),
        conduit_ai::LARGE_LOCAL_IMPLEMENTATION
    );
    let immutable_plan_a = plan_a.clone();

    let mut refreshed = initial_observations.clone();
    let unavailable = refreshed
        .iter_mut()
        .find(|observation| {
            observation.host_id.as_str() == "ai-large-local"
                && observation.class_id.as_str() == conduit_ai::ACCELERATOR_SLOT_RESOURCE
        })
        .expect("large-local accelerator observation exists");
    unavailable.health = ResourceHealth::Unavailable;
    unavailable.unreserved_units = 0;
    let remote = &fixtures[2].advertisement;
    let authority = conduit_core::authority_grant(
        "remote-generate-text-replan-grant",
        &remote.capabilities[0].authority_requirements[0],
        remote.host_id.clone(),
        remote.boot_id.clone(),
        remote.capabilities[0].capability_id.clone(),
    );
    let connection_choices = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let outcome = replan_selected_realizations_with_characteristics(
        &plan_a,
        &form,
        &hosts,
        &[],
        &requirements,
        &advertisements,
        &refreshed,
        &policies,
        PlanningOptions {
            connection_bases: &connection_choices,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[authority],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("refreshed observations admit a separately planned remote realization");
    let RealizationReplanOutcome::Replacement {
        previous_plan_id,
        plan: plan_b,
    } = outcome
    else {
        panic!("changed availability must produce a replacement Plan");
    };
    assert_eq!(plan_a, immutable_plan_a);
    assert_eq!(previous_plan_id, plan_a.plan_id);
    assert_ne!(plan_b.plan_id, plan_a.plan_id);
    assert_eq!(
        plan_b.fragments[0].placements[0].implementation_id.as_str(),
        conduit_ai::REMOTE_FRONTIER_IMPLEMENTATION
    );
    assert_eq!(plan_b.fragments[0].placements[0].authority.len(), 1);

    let unchanged = replan_selected_realizations_with_characteristics(
        &plan_a,
        &form,
        &hosts,
        &[],
        &requirements,
        &advertisements,
        &initial_observations,
        &policies,
        PlanningOptions {
            connection_bases: &connection_choices,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("unchanged observations preserve Plan identity");
    assert_eq!(
        unchanged,
        RealizationReplanOutcome::Unchanged {
            plan_id: plan_a.plan_id.clone()
        }
    );
}
