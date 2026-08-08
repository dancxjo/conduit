use conduit_ai::{
    generate_text_provider_fixtures, generate_text_realization_advertisements,
    install_generate_text_catalog, DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC,
    METERED_COST_CHARACTERISTIC,
};
use conduit_core::{EvidenceId, RealizationCharacteristicId, ResourceHealth, ResourceObservation};
use conduit_planner::{
    plan_selected_realizations_with_characteristics,
    select_realization_with_characteristics_and_evidence, HardRealizationRequirements,
    RealizationDecisionDisposition, RealizationPolicy, RealizationPreference, RealizationRejection,
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

fn observations(realm: &[conduit_core::HostAdvertisement]) -> Vec<ResourceObservation> {
    realm
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
                    evidence_id: EvidenceId::from(format!(
                        "{}-observation-{index}",
                        host.host_id.as_str()
                    )),
                })
        })
        .collect()
}

#[test]
fn context_and_privacy_hard_requirements_select_only_large_local() {
    let form = form();
    let fixtures = generate_text_provider_fixtures();
    let realm = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let operation = &form.operations[0];
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
    let requirement_map = BTreeMap::from([(operation.operation_id.clone(), requirements)]);
    let plan = plan_selected_realizations_with_characteristics(
        &form,
        &realm,
        &[],
        &requirement_map,
        &advertisements,
        &observations(&realm),
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
        &realm,
        &[],
        &requirement_map,
        &changed_advertisements,
        &observations(&realm),
        &BTreeMap::new(),
    )
    .expect("changed stable fact replans");
    assert_ne!(plan.plan_id, changed.plan_id);
}

#[test]
fn bounded_decision_evidence_explains_rejections_and_exact_selection() {
    let form = form();
    let fixtures = generate_text_provider_fixtures();
    let realm = fixtures
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

    let selection = select_realization_with_characteristics_and_evidence(
        &form.operations[0],
        &realm,
        &advertisements,
        &requirements,
        &observations(&realm),
        &RealizationPolicy::default(),
    )
    .expect("decision evidence accompanies selection");
    assert_eq!(selection.choice.host_id.as_str(), "ai-large-local");
    assert_eq!(selection.evidence.len(), 3);
    let small = selection
        .evidence
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
        .evidence
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
        .evidence
        .iter()
        .find(|record| record.host_id.as_str() == "ai-remote-provider")
        .expect("remote candidate is recorded");
    assert_eq!(
        remote.disposition,
        RealizationDecisionDisposition::Rejected(RealizationRejection::RequiredCharacteristicFlag(
            RealizationCharacteristicId::from(DATA_EGRESS_CHARACTERISTIC)
        ))
    );
}

#[test]
fn decision_evidence_fails_before_exceeding_its_candidate_bound() {
    let form = form();
    let fixture = generate_text_provider_fixtures()[0].clone();
    let mut realm = Vec::new();
    let mut advertisements = Vec::new();
    for index in 0..=conduit_planner::MAXIMUM_REALIZATION_DECISION_RECORDS {
        let mut host = fixture.advertisement.clone();
        host.host_id = conduit_core::HostId::from(format!("bounded-host-{index:03}"));
        host.boot_id = conduit_core::BootId::from(format!("bounded-boot-{index:03}"));
        host.capabilities[0].capability_id =
            conduit_core::CapabilityId::from(format!("bounded-capability-{index:03}"));
        let mut facts = conduit_ai::generate_text_realization_advertisements(&[
            conduit_ai::GenerateTextProviderFixture {
                advertisement: host.clone(),
                facts: fixture.facts.clone(),
            },
        ]);
        advertisements.push(facts.pop().expect("one realization fact"));
        realm.push(host);
    }
    let error = select_realization_with_characteristics_and_evidence(
        &form.operations[0],
        &realm,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&realm),
        &RealizationPolicy::default(),
    )
    .expect_err("candidate evidence cannot grow above its fixed planning bound");
    assert!(matches!(
        error,
        conduit_planner::PlannerError::PlannerLimitExceeded(_)
    ));
}

#[test]
fn explicit_policy_can_prefer_remote_among_hard_admissible_candidates() {
    let form = form();
    let fixtures = generate_text_provider_fixtures();
    let realm = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let selection = select_realization_with_characteristics_and_evidence(
        &form.operations[0],
        &realm,
        &advertisements,
        &HardRealizationRequirements::default(),
        &observations(&realm),
        &RealizationPolicy {
            preferences: vec![RealizationPreference::PreferCharacteristicFlag {
                characteristic_id: RealizationCharacteristicId::from(METERED_COST_CHARACTERISTIC),
                value: true,
            }],
        },
    )
    .expect("explicit policy selects a metered candidate");
    assert_eq!(selection.choice.host_id.as_str(), "ai-remote-provider");
    assert_eq!(
        selection
            .evidence
            .iter()
            .filter(|record| record.disposition == RealizationDecisionDisposition::Admitted)
            .count(),
        2
    );
}
