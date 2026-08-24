use conduit_ai::{DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC};
use conduit_core::{
    CharacteristicId, CharacteristicUnit, ConnectionBase, ResourceHealth, ResourceObservation,
    SignId,
};
use conduit_planner::{
    seal_reviewed_service_profile_plan, select_reviewed_service_profile, DegradationDirection,
    DegradedDimension, HardRealizationRequirements, PlannerFactRef, PlannerFactValue,
    PlannerPredicate, RealizationPolicy, ReviewedServiceProfile, SurvivalPolicy,
};
use patchbay_model::{
    explain_degraded_profile, explain_degraded_profile_refusal, DegradedProfileState,
};

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
                    sign_id: SignId::from(format!("{}/observation/{index}", host.host_id.as_str())),
                })
        })
        .collect()
}

fn form() -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_ai::install_generate_text_catalog(&mut startup, &mut profile).unwrap();
    conduit_form::parse("form answer {\n generate: ai/generate-text\n}\n", &profile).unwrap()
}

fn profile() -> ReviewedServiceProfile {
    ReviewedServiceProfile {
        profile_id: "ai/generate-text/survival@1".into(),
        hard_requirements: HardRealizationRequirements {
            predicates: vec![PlannerPredicate::Equal {
                fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                    DATA_EGRESS_CHARACTERISTIC,
                )),
                value: PlannerFactValue::Boolean(false),
            }],
            ..HardRealizationRequirements::default()
        },
        required_evidence: None,
        degradable_dimensions: vec![DegradedDimension {
            characteristic_id: CharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC),
            human_name: "maximum context".into(),
            full_value: PlannerFactValue::Quantity {
                value: 32_768,
                unit: CharacteristicUnit::Tokens,
            },
            weakest_permitted_value: PlannerFactValue::Quantity {
                value: 8_192,
                unit: CharacteristicUnit::Tokens,
            },
            direction: DegradationDirection::HigherIsStronger,
        }],
    }
}

fn policy() -> SurvivalPolicy {
    SurvivalPolicy {
        policy_id: "policy/voyager/context-survival".into(),
        revision: 1,
        permitted_profile_id: "ai/generate-text/survival@1".into(),
        permitted_dimensions: vec![CharacteristicId::from(MAXIMUM_CONTEXT_CHARACTERISTIC)],
        degradation_allowed: true,
    }
}

#[test]
fn patchbay_names_requested_surviving_policy_plan_and_current_signs() {
    let form = form();
    let fixtures = conduit_ai::generate_text_base_fixtures();
    let hosts = fixtures
        .iter()
        .map(|item| item.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = conduit_ai::generate_text_realization_advertisements(&fixtures);
    let full = select_reviewed_service_profile(
        &form.gears[0],
        &hosts,
        &advertisements,
        &observations(&hosts),
        &profile(),
        Some(&policy()),
        &RealizationPolicy::default(),
    )
    .unwrap();
    let plan_a = seal_reviewed_service_profile_plan(
        &form,
        &hosts,
        &[ConnectionBase::Local],
        &advertisements,
        &full,
    )
    .unwrap();
    let surviving_hosts = hosts
        .into_iter()
        .filter(|host| host.host_id.as_str() == "ai-small-local")
        .collect::<Vec<_>>();
    let surviving_ads = advertisements
        .into_iter()
        .filter(|item| item.host_id.as_str() == "ai-small-local")
        .collect::<Vec<_>>();
    let degraded = select_reviewed_service_profile(
        &form.gears[0],
        &surviving_hosts,
        &surviving_ads,
        &observations(&surviving_hosts),
        &profile(),
        Some(&policy()),
        &RealizationPolicy::default(),
    )
    .unwrap();
    let plan_b = seal_reviewed_service_profile_plan(
        &form,
        &surviving_hosts,
        &[ConnectionBase::Local],
        &surviving_ads,
        &degraded,
    )
    .unwrap();
    let explanation = explain_degraded_profile(Some(&plan_a), &plan_b, &degraded).unwrap();
    assert_eq!(explanation.state, DegradedProfileState::Degraded);
    assert_eq!(
        explanation.previous_plan_id.as_deref(),
        Some(plan_a.plan_id.as_str())
    );
    assert_eq!(explanation.plan_id, plan_b.plan_id.as_str());
    assert_eq!(
        explanation.policy_id.as_deref(),
        Some("policy/voyager/context-survival")
    );
    assert_eq!(explanation.dimensions[0].requested, "32768 Tokens");
    assert_eq!(explanation.dimensions[0].surviving, "8192 Tokens");
    assert!(!explanation.observation_signs.is_empty());
    assert!(!explanation.hard_requirements_relaxed);
    assert!(explanation
        .summary
        .contains("Hard requirements were not relaxed"));

    let (text, state) = explain_degraded_profile_refusal(
        &conduit_planner::DegradedProfileRefusal::HardRequirementUnsatisfied,
    );
    assert_eq!(state, DegradedProfileState::Unrealizable);
    assert_eq!(text, "hard requirement unsatisfied");
}
