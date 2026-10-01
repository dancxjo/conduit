use super::*;
use conduit_core::{
    bind_active_play, bind_sign, seal_plan_with_realization_backs_and_completion,
    ExternalEffectBehavior, FormIdentity, KindSemanticLaw, ReplayBehavior, SemanticDependence,
    SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
};
use conduit_planner::{
    admit_realization_recovery, RealizationInvalidation, RealizationRecoveryOutcome,
    RealizationRecoveryRefusal, RealizationReplanOutcome, RecoveryPlanningOutcome,
};

fn pure_laws() -> Vec<KindSemanticLaw> {
    vec![
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::None),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]
}

fn selected_http(plan: &conduit_core::Plan) -> &conduit_core::PlannedGear {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == HTTP)
        .expect("HTTP placement")
}

fn selected_lines(
    plan: &conduit_core::Plan,
    placement_id: &conduit_core::PlacementId,
) -> Vec<LineId> {
    let mut lines = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .filter(|connection| {
            &connection.source_placement_id == placement_id
                || &connection.sink_placement_id == placement_id
        })
        .filter_map(|connection| connection.selected_line.as_ref())
        .map(|line| line.line_id.clone())
        .collect::<Vec<_>>();
    lines.sort();
    lines.dedup();
    lines
}

fn with_pure_http_contract(mut plan: conduit_core::Plan) -> conduit_core::Plan {
    for placement in plan
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .filter(|placement| placement.kind_id.as_str() == HTTP)
    {
        placement.semantic_contract.laws = pure_laws();
    }
    seal_plan_with_realization_backs_and_completion(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        plan.completion_policy,
        plan.realization_backs,
        plan.fragments,
    )
}

fn invalidation(
    plan: &conduit_core::Plan,
    play: &conduit_core::ActivePlayIdentity,
) -> RealizationInvalidation {
    let placement = selected_http(plan);
    RealizationInvalidation {
        placement_id: placement.placement_id.clone(),
        host_id: placement.host_id.clone(),
        boot_id: placement.boot_id.clone(),
        offer_generation: placement.offer_generation,
        implementation_id: placement.implementation_id.clone(),
        selected_line_ids: selected_lines(plan, &placement.placement_id),
        supporting_sign: bind_sign(&play.host_id, &play.boot_id, Some(&play.active_play_id), 9),
    }
}

#[test]
fn exact_loss_and_checked_move_admit_a_distinct_plan_and_play() {
    let (_, plan_a) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let (_, plan_b) = plan_with_http_part(host("c", &[HTTP, DECODE]));
    let plan_b = with_pure_http_contract(plan_b);
    let old = selected_http(&plan_a);
    let replacement = selected_http(&plan_b);
    let play_a = bind_active_play(&plan_a.plan_id, &old.host_id, &old.boot_id, 3);
    let play_b = bind_active_play(
        &plan_b.plan_id,
        &replacement.host_id,
        &replacement.boot_id,
        4,
    );
    let outcome = admit_realization_recovery(
        &plan_a,
        &play_a,
        invalidation(&plan_a, &play_a),
        RecoveryPlanningOutcome::Planned(RealizationReplanOutcome::Replacement {
            previous_plan_id: plan_a.plan_id.clone(),
            plan: plan_b.clone(),
        }),
        Some(play_b.clone()),
    )
    .expect("exact pure movement is admitted");

    let RealizationRecoveryOutcome::Replacement { plan, evidence } = outcome else {
        panic!("available admitted alternative must replace")
    };
    assert_eq!(plan.plan_id, plan_b.plan_id);
    assert_ne!(evidence.previous_plan_id, plan.plan_id);
    assert_ne!(evidence.previous_play.active_play_id, play_b.active_play_id);
    assert_eq!(evidence.replacement_play.as_ref(), Some(&play_b));
    assert_eq!(
        evidence.invalidation.implementation_id,
        old.implementation_id
    );
    assert!(!evidence.invalidation.selected_line_ids.is_empty());
    assert_eq!(
        evidence.accept_completion(&play_a),
        Err(RealizationRecoveryRefusal::StaleCompletion)
    );
    assert_eq!(evidence.accept_completion(&play_b), Ok(()));
}

#[test]
fn unknown_replay_law_refuses_before_a_replacement_can_be_blessed() {
    let (_, plan_a) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let (_, plan_b) = plan_with_http_part(host("c", &[HTTP, DECODE]));
    let old = selected_http(&plan_a);
    let replacement = selected_http(&plan_b);
    let play_a = bind_active_play(&plan_a.plan_id, &old.host_id, &old.boot_id, 3);
    let play_b = bind_active_play(
        &plan_b.plan_id,
        &replacement.host_id,
        &replacement.boot_id,
        4,
    );
    assert!(matches!(
        admit_realization_recovery(
            &plan_a,
            &play_a,
            invalidation(&plan_a, &play_a),
            RecoveryPlanningOutcome::Planned(RealizationReplanOutcome::Replacement {
                previous_plan_id: plan_a.plan_id.clone(),
                plan: plan_b,
            }),
            Some(play_b),
        ),
        Err(RealizationRecoveryRefusal::Transformation(_))
    ));
}

#[test]
fn no_admitted_alternative_is_an_explicit_terminal_not_a_hidden_retry() {
    let (_, plan_a) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let old = selected_http(&plan_a);
    let play_a = bind_active_play(&plan_a.plan_id, &old.host_id, &old.boot_id, 3);
    let outcome = admit_realization_recovery(
        &plan_a,
        &play_a,
        invalidation(&plan_a, &play_a),
        RecoveryPlanningOutcome::NoAdmittedAlternative {
            reason: "no semantically substitutable admitted Back remains".into(),
        },
        None,
    )
    .expect("ordinary exhaustion is represented exactly");
    let RealizationRecoveryOutcome::NoAdmittedAlternative { reason, evidence } = outcome else {
        panic!("loss without B must not claim replacement")
    };
    assert!(reason.contains("no semantically substitutable"));
    assert_eq!(evidence.replacement_plan_id, None);
    assert_eq!(evidence.replacement_play, None);
    assert_eq!(
        evidence.accept_completion(&play_a),
        Err(RealizationRecoveryRefusal::StaleCompletion)
    );
}

#[test]
fn mismatched_provider_or_line_truth_cannot_invalidate_the_selected_realization() {
    let (_, plan_a) = plan_with_http_part(host("b", &[HTTP, DECODE]));
    let old = selected_http(&plan_a);
    let play_a = bind_active_play(&plan_a.plan_id, &old.host_id, &old.boot_id, 3);
    let mut wrong = invalidation(&plan_a, &play_a);
    wrong.implementation_id = ImplementationId::from("test/not-selected@1");
    assert_eq!(
        admit_realization_recovery(
            &plan_a,
            &play_a,
            wrong,
            RecoveryPlanningOutcome::NoAdmittedAlternative {
                reason: "none".into()
            },
            None,
        ),
        Err(RealizationRecoveryRefusal::InvalidationDoesNotMatchPlan)
    );

    let mut wrong = invalidation(&plan_a, &play_a);
    wrong.selected_line_ids.clear();
    assert_eq!(
        admit_realization_recovery(
            &plan_a,
            &play_a,
            wrong,
            RecoveryPlanningOutcome::NoAdmittedAlternative {
                reason: "none".into()
            },
            None,
        ),
        Err(RealizationRecoveryRefusal::InvalidLineEvidence)
    );
}
