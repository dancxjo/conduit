use conduit_ai::{DATA_EGRESS_CHARACTERISTIC, MAXIMUM_CONTEXT_CHARACTERISTIC};
use conduit_core::{CharacteristicId, SignId};
use conduit_planner::{
    select_realization_with_scoped_policy, HardRealizationRequirements, PlannerError,
    PlannerFactRef, PlannerFactValue, PlannerPredicate, PlannerPreference, PolicyLayer,
    PolicyScope, PolicySourceId, PolicySourceRevision, RealizationDecisionDisposition,
    RealizationPreference, ReviewedObservation,
};

mod common;
use common::{generic_policy_facts as facts, resource_observations as observations};

fn source(id: &str, revision: u64, scope: PolicyScope) -> PolicySourceRevision {
    PolicySourceRevision {
        source_id: PolicySourceId::from(id),
        revision,
        scope,
    }
}

fn reviewed(
    hosts: &[conduit_core::HostAdvertisement],
    observed_epoch: u64,
    valid_through_epoch: u64,
) -> Vec<ReviewedObservation> {
    observations(hosts)
        .into_iter()
        .map(|observation| ReviewedObservation {
            observation,
            source: source("resource-monitor", 7, PolicyScope::SiteDeployment),
            observed_epoch,
            valid_through_epoch,
        })
        .collect()
}

#[test]
fn explicit_precedence_is_lexicographic_and_evidence_names_each_source() {
    let (form, hosts, advertisements) = facts();
    let semantic = source("checked-form", 3, PolicyScope::SemanticRequirements);
    let site = source("site-policy", 4, PolicyScope::SiteDeployment);
    let style = source("fixed-cell", 2, PolicyScope::NamedStyle);
    let egress = PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
        DATA_EGRESS_CHARACTERISTIC,
    ));
    let context = PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
        MAXIMUM_CONTEXT_CHARACTERISTIC,
    ));
    let result = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements {
            predicates: vec![PlannerPredicate::Equal {
                fact: egress.clone(),
                value: PlannerFactValue::Boolean(false),
            }],
            ..HardRealizationRequirements::default()
        },
        semantic.clone(),
        &[
            PolicyLayer {
                source: site.clone(),
                hard_predicates: vec![],
                preferences: vec![RealizationPreference::Fact(PlannerPreference::Maximize {
                    fact: context,
                })],
            },
            PolicyLayer {
                source: style.clone(),
                hard_predicates: vec![],
                preferences: vec![RealizationPreference::Fact(
                    PlannerPreference::PreferEqual {
                        fact: egress,
                        value: PlannerFactValue::Boolean(false),
                    },
                )],
            },
        ],
        &reviewed(&hosts, 10, 12),
        11,
    )
    .expect("explicit scoped policy selects through the ordinary evaluator");

    assert_eq!(result.selection.choice.host_id.as_str(), "ai-large-local");
    let rejected = result
        .selection
        .signs
        .iter()
        .find(|record| record.host_id.as_str() == "ai-remote-base")
        .expect("remote rejection evidence exists");
    assert_eq!(rejected.clause_source.as_ref(), Some(&semantic));
    let selected = result
        .selection
        .signs
        .iter()
        .find(|record| record.disposition == RealizationDecisionDisposition::Selected)
        .expect("selected evidence exists");
    assert_eq!(selected.decisive_preference_clause, Some(1));
    assert_eq!(selected.decisive_preference_source.as_ref(), Some(&site));
    assert_eq!(
        result.basis.policy_sources,
        vec![semantic, style, site],
        "the inspectable basis follows explicit precedence, not input order"
    );
    assert!(result.basis.observations.iter().all(|item| {
        item.observed_epoch == 10
            && item.valid_through_epoch == 12
            && !item.sign_id.as_str().is_empty()
    }));
}

#[test]
fn conflicting_hard_sources_refuse_before_candidate_choice() {
    let (form, hosts, advertisements) = facts();
    let fact = PlannerFactRef::RealizationCharacteristic(CharacteristicId::from(
        DATA_EGRESS_CHARACTERISTIC,
    ));
    let error = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        source("semantic", 1, PolicyScope::SemanticRequirements),
        &[
            PolicyLayer {
                source: source("site", 1, PolicyScope::SiteDeployment),
                hard_predicates: vec![PlannerPredicate::Equal {
                    fact: fact.clone(),
                    value: PlannerFactValue::Boolean(false),
                }],
                preferences: vec![],
            },
            PolicyLayer {
                source: source("one-shot", 1, PolicyScope::OneShotOverride),
                hard_predicates: vec![PlannerPredicate::Equal {
                    fact,
                    value: PlannerFactValue::Boolean(true),
                }],
                preferences: vec![],
            },
        ],
        &reviewed(&hosts, 1, 1),
        1,
    )
    .expect_err("incompatible hard constraints cannot be resolved by precedence");
    assert!(matches!(
        error,
        PlannerError::InvalidHardRealizationRequirement(_)
    ));
}

#[test]
fn stale_observations_are_retained_but_cannot_enter_a_fresh_basis() {
    let (form, hosts, advertisements) = facts();
    let error = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        source("semantic", 1, PolicyScope::SemanticRequirements),
        &[],
        &reviewed(&hosts, 1, 2),
        3,
    )
    .expect_err("stale observations cannot admit current resources");
    assert!(matches!(
        error,
        PlannerError::CurrentResourceObservationUnavailable(_)
    ));
}

#[test]
fn policy_revision_changes_evidence_without_perturbing_realization_truth() {
    let (form, hosts, advertisements) = facts();
    let make_selection = |revision| {
        select_realization_with_scoped_policy(
            &form.gears[0],
            &hosts,
            &advertisements,
            &HardRealizationRequirements::default(),
            source("semantic", 1, PolicyScope::SemanticRequirements),
            &[PolicyLayer {
                source: source("workspace", revision, PolicyScope::UserWorkspace),
                hard_predicates: vec![],
                preferences: vec![RealizationPreference::MaximizeQueueItems],
            }],
            &reviewed(&hosts, 5, 5),
            5,
        )
        .expect("policy revision is valid")
    };
    let first = make_selection(7);
    let second = make_selection(8);
    assert_eq!(first.selection.choice, second.selection.choice);
    assert_ne!(first.basis.policy_sources, second.basis.policy_sources);
    assert_eq!(
        first.basis.policy_sources[1],
        source("workspace", 7, PolicyScope::UserWorkspace)
    );
}

#[test]
fn observation_signs_must_be_unique_even_across_retained_history() {
    let (form, hosts, advertisements) = facts();
    let mut history = reviewed(&hosts, 1, 1);
    let mut duplicate = history[0].clone();
    duplicate.observed_epoch = 2;
    duplicate.valid_through_epoch = 2;
    duplicate.observation.sign_id = SignId::from(history[0].observation.sign_id.as_str());
    history.push(duplicate);
    let error = select_realization_with_scoped_policy(
        &form.gears[0],
        &hosts,
        &advertisements,
        &HardRealizationRequirements::default(),
        source("semantic", 1, PolicyScope::SemanticRequirements),
        &[],
        &history,
        2,
    )
    .expect_err("a retained Sign identity cannot be replayed");
    assert!(matches!(error, PlannerError::InvalidResourceObservation(_)));
}
