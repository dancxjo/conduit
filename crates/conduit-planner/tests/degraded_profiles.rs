use conduit_ai::{DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC};
use conduit_core::{CharacteristicId, CharacteristicUnit, ConnectionBase};
use conduit_planner::{
    seal_reviewed_service_profile_plan, select_reviewed_service_profile, DegradationDirection,
    DegradedDimension, DegradedProfileRefusal, HardRealizationRequirements, PlannerFactRef,
    PlannerFactValue, PlannerPredicate, RealizationPolicy, ReviewedServiceProfile,
    ServiceProfileDisposition, SurvivalPolicy,
};

mod common;
use common::{generic_policy_facts as facts, quantity, resource_observations as observations};

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
            full_value: quantity(32_768, CharacteristicUnit::Tokens),
            weakest_permitted_value: quantity(8_192, CharacteristicUnit::Tokens),
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
fn full_loss_admits_exact_weaker_profile_and_seals_a_fresh_plan() {
    let (form, hosts, advertisements) = facts();
    let all_observations = observations(&hosts);
    let full = select_reviewed_service_profile(
        &form.gears[0],
        &hosts,
        &advertisements,
        &all_observations,
        &profile(),
        Some(&policy()),
        &RealizationPolicy::default(),
    )
    .unwrap();
    assert_eq!(full.disposition, ServiceProfileDisposition::Full);
    assert_eq!(full.choice.host_id.as_str(), "ai-large-local");
    assert!(full.policy_id.is_none());
    let plan_a = seal_reviewed_service_profile_plan(
        &form,
        &hosts,
        &[ConnectionBase::Local],
        &advertisements,
        &full,
    )
    .unwrap();
    let plan_a_snapshot = plan_a.clone();

    let surviving_hosts = hosts
        .iter()
        .filter(|host| host.host_id.as_str() == "ai-small-local")
        .cloned()
        .collect::<Vec<_>>();
    let surviving_advertisements = advertisements
        .iter()
        .filter(|item| item.host_id.as_str() == "ai-small-local")
        .cloned()
        .collect::<Vec<_>>();
    let degraded = select_reviewed_service_profile(
        &form.gears[0],
        &surviving_hosts,
        &surviving_advertisements,
        &observations(&surviving_hosts),
        &profile(),
        Some(&policy()),
        &RealizationPolicy::default(),
    )
    .unwrap();
    assert_eq!(degraded.disposition, ServiceProfileDisposition::Degraded);
    assert_eq!(degraded.choice.host_id.as_str(), "ai-small-local");
    assert_eq!(
        degraded.policy_id.as_deref(),
        Some("policy/voyager/context-survival")
    );
    assert_eq!(
        degraded.dimensions[0].requested_value,
        quantity(32_768, CharacteristicUnit::Tokens)
    );
    assert_eq!(
        degraded.dimensions[0].admitted_value,
        quantity(8_192, CharacteristicUnit::Tokens)
    );
    assert!(!degraded.observation_signs.is_empty());
    let plan_b = seal_reviewed_service_profile_plan(
        &form,
        &surviving_hosts,
        &[ConnectionBase::Local],
        &surviving_advertisements,
        &degraded,
    )
    .unwrap();
    assert_eq!(plan_a, plan_a_snapshot);
    assert_ne!(plan_a.plan_id, plan_b.plan_id);
    assert_eq!(plan_a.source_document_id, plan_b.source_document_id);
    assert_eq!(plan_a.checked_form_id, plan_b.checked_form_id);
    assert_eq!(plan_a.expanded_form_id, plan_b.expanded_form_id);
}

#[test]
fn hard_policy_staleness_and_unreviewed_relaxation_refuse_distinctly() {
    let (form, hosts, advertisements) = facts();
    let small_hosts = hosts
        .iter()
        .filter(|host| host.host_id.as_str() == "ai-small-local")
        .cloned()
        .collect::<Vec<_>>();
    let small_ads = advertisements
        .iter()
        .filter(|item| item.host_id.as_str() == "ai-small-local")
        .cloned()
        .collect::<Vec<_>>();
    let small_observations = observations(&small_hosts);
    let select =
        |profile: &ReviewedServiceProfile, policy: Option<&SurvivalPolicy>, observations| {
            select_reviewed_service_profile(
                &form.gears[0],
                &small_hosts,
                &small_ads,
                observations,
                profile,
                policy,
                &RealizationPolicy::default(),
            )
        };
    assert_eq!(
        select(&profile(), None, &small_observations),
        Err(DegradedProfileRefusal::DegradationForbidden)
    );
    let mut denied = policy();
    denied.degradation_allowed = false;
    assert_eq!(
        select(&profile(), Some(&denied), &small_observations),
        Err(DegradedProfileRefusal::DegradationForbidden)
    );
    let mut outside = policy();
    outside.permitted_dimensions = vec![CharacteristicId::from("other/quality@1")];
    assert_eq!(
        select(&profile(), Some(&outside), &small_observations),
        Err(DegradedProfileRefusal::PolicyOutsideReviewedBounds)
    );
    let mut impossible = profile();
    impossible.hard_requirements.predicates[0] = PlannerPredicate::Equal {
        fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
            DATA_EGRESS_CHARACTERISTIC,
        )),
        value: PlannerFactValue::Boolean(true),
    };
    assert_eq!(
        select(&impossible, Some(&policy()), &small_observations),
        Err(DegradedProfileRefusal::HardRequirementUnsatisfied)
    );
    let mut stale = small_observations.clone();
    stale[0].boot_id = conduit_core::BootId::from("stale-boot");
    assert_eq!(
        select(&profile(), Some(&policy()), &stale),
        Err(DegradedProfileRefusal::StaleOrMissingObservation)
    );
}

#[test]
fn semantic_units_evidence_and_absent_weaker_profiles_never_launder() {
    let (form, hosts, advertisements) = facts();
    let small_hosts = hosts
        .iter()
        .filter(|host| host.host_id.as_str() == "ai-small-local")
        .cloned()
        .collect::<Vec<_>>();
    let mut small_ads = advertisements
        .iter()
        .filter(|item| item.host_id.as_str() == "ai-small-local")
        .cloned()
        .collect::<Vec<_>>();
    let observed = observations(&small_hosts);
    let mut wrong_unit = profile();
    wrong_unit.degradable_dimensions[0].weakest_permitted_value =
        quantity(8_192, CharacteristicUnit::Bytes);
    assert_eq!(
        select_reviewed_service_profile(
            &form.gears[0],
            &small_hosts,
            &small_ads,
            &observed,
            &wrong_unit,
            Some(&policy()),
            &RealizationPolicy::default()
        ),
        Err(DegradedProfileRefusal::SemanticallyDifferentDimension)
    );
    let mut evidence = profile();
    small_ads[0]
        .characteristics
        .push(conduit_core::stable_realization_category(
            "conduit.realization/evidence-class@1",
            "Evidence class",
            "Reviewed provenance class for generated output.",
            vec!["inferred".into(), "measured".into()],
            false,
            "inferred",
        ));
    evidence.required_evidence = Some((
        CharacteristicId::from("conduit.realization/evidence-class@1"),
        PlannerFactValue::Category("measured".into()),
    ));
    assert_eq!(
        select_reviewed_service_profile(
            &form.gears[0],
            &small_hosts,
            &small_ads,
            &observed,
            &evidence,
            Some(&policy()),
            &RealizationPolicy::default()
        ),
        Err(DegradedProfileRefusal::MissingRequiredEvidence)
    );
    let mut no_fit = profile();
    no_fit.degradable_dimensions[0].weakest_permitted_value =
        quantity(9_000, CharacteristicUnit::Tokens);
    assert_eq!(
        select_reviewed_service_profile(
            &form.gears[0],
            &small_hosts,
            &small_ads,
            &observed,
            &no_fit,
            Some(&policy()),
            &RealizationPolicy::default()
        ),
        Err(DegradedProfileRefusal::Unrealizable)
    );
}
