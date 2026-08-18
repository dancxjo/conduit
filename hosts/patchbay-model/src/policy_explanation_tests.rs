use std::collections::BTreeMap;

use conduit_ai::{DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC};
use conduit_core::{CharacteristicId, CharacteristicUnit, PlanId};
use conduit_planner::{
    plan, select_realization_with_scoped_policy, HardRealizationRequirements, PlacementChoices,
    PlannerFactRef, PlannerFactValue, PlannerPredicate, PlannerPreference, PolicyLayer,
    PolicyScope, PolicySourceId, PolicySourceRevision, RealizationPreference, ReviewedObservation,
    StylePreferenceEvidence, StylePreferenceOutcome,
};

use crate::{
    PatchbayPresentation, PolicyChoiceDomain, PolicyChoiceExplanation, PolicyExplanationError,
};

fn source(id: &str, scope: PolicyScope) -> PolicySourceRevision {
    PolicySourceRevision {
        source_id: PolicySourceId::from(id),
        revision: 1,
        scope,
    }
}

fn fixture() -> (
    conduit_form::CheckedForm,
    Vec<conduit_core::HostAdvertisement>,
    conduit_planner::ScopedRealizationSelection,
    conduit_core::Plan,
) {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_ai::install_generate_text_catalog(&mut startup, &mut profile).unwrap();
    let form = conduit_form::parse(
        "form answer {\n    generate: ai/generate-text\n}\n",
        &profile,
    )
    .unwrap();
    let fixtures = conduit_ai::generate_text_base_fixtures();
    let hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let advertisements = conduit_ai::generate_text_realization_advertisements(&fixtures);
    let observations = hosts
        .iter()
        .flat_map(|host| {
            host.resources
                .iter()
                .enumerate()
                .map(move |(index, pool)| ReviewedObservation {
                    observation: conduit_core::ResourceObservation {
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.clone(),
                        offer_generation: host.offer_generation,
                        pool_id: pool.pool_id.clone(),
                        class_id: pool.class_id.clone(),
                        health: conduit_core::ResourceHealth::Ready,
                        unreserved_units: pool.capacity_units,
                        utilized_units: 0,
                        sign_id: conduit_core::SignId::from(format!(
                            "{}-policy-{index}",
                            host.host_id.as_str()
                        )),
                    },
                    source: source("resource-monitor", PolicyScope::SiteDeployment),
                    observed_epoch: 7,
                    valid_through_epoch: 7,
                })
        })
        .collect::<Vec<_>>();
    let context = PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
        MAXIMUM_CONTEXT_CHARACTERISTIC,
    ));
    let selection = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates: vec![
                PlannerPredicate::AtLeast {
                    fact: context.clone(),
                    value: PlannerFactValue::Quantity {
                        value: 24_000,
                        unit: CharacteristicUnit::Tokens,
                    },
                },
                PlannerPredicate::Equal {
                    fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                        DATA_EGRESS_CHARACTERISTIC,
                    )),
                    value: PlannerFactValue::Boolean(false),
                },
            ],
            ..HardRealizationRequirements::default()
        },
        source("checked-form", PolicyScope::SemanticRequirements),
        &[PolicyLayer {
            source: source("workspace-policy", PolicyScope::UserWorkspace),
            hard_predicates: Vec::new(),
            preferences: vec![RealizationPreference::Fact(PlannerPreference::Maximize {
                fact: context,
            })],
        }],
        &observations,
        7,
    )
    .unwrap();
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([(
            form.gears[0].gear_id.clone(),
            selection.selection.choice.clone(),
        )]),
    };
    let plan = plan(&form, &hosts, &placements, &[]).unwrap();
    (form, hosts, selection, plan)
}

fn explanation(
    domain: PolicyChoiceDomain,
    selection: &conduit_planner::ScopedRealizationSelection,
    plan: &conduit_core::Plan,
    gear_id: &conduit_core::GearId,
) -> crate::PolicyChoiceExplanation {
    PolicyChoiceExplanation::from_planner_evidence(
        plan,
        gear_id,
        domain,
        match domain {
            PolicyChoiceDomain::Realization => "LLM",
            PolicyChoiceDomain::ComputeResource => "Compute",
            PolicyChoiceDomain::PresentationStyle => "Presentation",
        },
        match domain {
            PolicyChoiceDomain::Realization => "Local model",
            PolicyChoiceDomain::ComputeResource => "Performance pool",
            PolicyChoiceDomain::PresentationStyle => "Native presenter",
        },
        "first preferred admissible match",
        selection.selection.signs.clone(),
        vec![PlannerFactRef::RealizationCharacteristic(
            CharacteristicId::from(DATA_EGRESS_CHARACTERISTIC),
        )],
        selection.basis.clone(),
        (domain == PolicyChoiceDomain::PresentationStyle).then(|| {
            (
                conduit_planner::StyleId::from("conduit.style/dos-shell@1"),
                vec![
                    StylePreferenceEvidence {
                        clause_index: 0,
                        fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                            conduit_planner::PRESENTATION_TEXT_LAYOUT,
                        )),
                        outcome: StylePreferenceOutcome::Matched,
                    },
                    StylePreferenceEvidence {
                        clause_index: 1,
                        fact: PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
                            conduit_planner::PRESENTATION_PALETTE_CLASS,
                        )),
                        outcome: StylePreferenceOutcome::Unavailable,
                    },
                ],
            )
        }),
    )
    .unwrap()
}

#[test]
fn one_progressive_grammar_summarizes_three_domains_and_discloses_exact_details() {
    let (form, _, selection, plan) = fixture();
    for domain in [
        PolicyChoiceDomain::Realization,
        PolicyChoiceDomain::ComputeResource,
        PolicyChoiceDomain::PresentationStyle,
    ] {
        let explanation = explanation(domain, &selection, &plan, &form.gears[0].gear_id);
        assert!(!explanation.summary.selected_label.is_empty());
        assert!(!explanation.summary.reason.is_empty());
        assert!(
            !explanation
                .summary
                .selected_label
                .contains(plan.plan_id.as_str()),
            "front-door summary does not promote exact identity"
        );
        let details = explanation.details();
        assert_eq!(details.plan_id, plan.plan_id);
        assert_eq!(details.candidates.len(), 3);
        assert!(!details.policy_sources.is_empty());
        assert!(!details.observations.is_empty());
        assert!(!details.current_observation_signs.is_empty());
        assert!(details
            .candidates
            .iter()
            .map(PolicyChoiceExplanation::candidate_text)
            .all(|text| text.contains(':')));
        if domain == PolicyChoiceDomain::PresentationStyle {
            assert_eq!(
                explanation.summary.style_label.as_deref(),
                Some("dos shell")
            );
            assert_eq!(
                details
                    .style_preferences
                    .iter()
                    .map(PolicyChoiceExplanation::style_text)
                    .collect::<Vec<_>>(),
                vec![
                    "matched: REALIZATION presentation/text-layout",
                    "unavailable: REALIZATION presentation/palette-class",
                ],
                "textual STYLE outcomes do not rely on color"
            );
        }
    }
}

#[test]
fn policy_change_is_a_fresh_replan_request_and_never_mutates_the_active_plan() {
    let (form, _, selection, plan) = fixture();
    let original = plan.clone();
    let explanation = explanation(
        PolicyChoiceDomain::PresentationStyle,
        &selection,
        &plan,
        &form.gears[0].gear_id,
    );
    let request = explanation
        .request_replan(
            &plan.plan_id,
            source("conduit.style/dos-shell@1", PolicyScope::NamedStyle),
            Some(conduit_planner::StyleId::from("conduit.style/dos-shell@1")),
        )
        .unwrap();
    assert_eq!(request.prior_plan_id, plan.plan_id);
    assert_eq!(plan, original);
    assert_eq!(
        explanation.request_replan(
            &PlanId::from("stale-plan"),
            source("conduit.style/spacious@1", PolicyScope::NamedStyle),
            None,
        ),
        Err(PolicyExplanationError::StaleReplanBasis)
    );
}

#[test]
fn renderer_projection_accepts_only_bounded_explanations_for_its_exact_plan() {
    let (form, _, selection, plan) = fixture();
    let explanation = explanation(
        PolicyChoiceDomain::Realization,
        &selection,
        &plan,
        &form.gears[0].gear_id,
    );
    let editor = crate::FormEditor::from_source(
        "policy-explanation.conduit".into(),
        "form policy-explanation {\n    literal: text/literal(\"hello\")\n}\n".into(),
    )
    .unwrap();
    let document = editor.view();
    let projected = PatchbayPresentation::new(1, document, None, None, None, vec![]).unwrap();
    assert!(projected
        .clone()
        .with_policy_explanations(vec![explanation.clone()])
        .is_err());
    assert!(projected
        .with_policy_explanations(vec![explanation; crate::MAX_POLICY_EXPLANATIONS + 1])
        .is_err());
}
