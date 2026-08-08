use conduit_ai::{
    generate_text_provider_fixtures, generate_text_realization_advertisements,
    install_generate_text_catalog, DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC,
    METERED_COST_CHARACTERISTIC,
};
use conduit_core::{EvidenceId, RealizationCharacteristicId, ResourceHealth, ResourceObservation};
use conduit_planner::{
    plan_selected_realizations_with_characteristics, select_realization_with_characteristics,
    HardRealizationRequirements, RealizationPolicy, RealizationPreference,
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
fn explicit_policy_can_prefer_remote_among_hard_admissible_candidates() {
    let form = form();
    let fixtures = generate_text_provider_fixtures();
    let realm = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = generate_text_realization_advertisements(&fixtures);
    let selected = select_realization_with_characteristics(
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
    assert_eq!(selected.host_id.as_str(), "ai-remote-provider");
}
