//! Cache/cold equivalence for real checked Source and cross-Host Line admission.
mod common;
#[path = "common/fusion_fixture.rs"]
mod fixture;
use conduit_core::*;
use conduit_planner::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

struct Facts {
    fixture: fixture::Fixture,
    line_present: bool,
    remote_present: bool,
}
impl Facts {
    fn new() -> Self {
        Self {
            fixture: fixture::fixture(),
            line_present: true,
            remote_present: true,
        }
    }
    fn basis(&self) -> Vec<PlanningFact> {
        let contents = [
            serde_json::to_string(&self.fixture.hosts[0]).unwrap(),
            serde_json::to_string(&self.remote_present.then_some(&self.fixture.hosts[1])).unwrap(),
            serde_json::to_string(&self.line_present.then_some(&self.fixture.line)).unwrap(),
        ];
        contents
            .into_iter()
            .enumerate()
            .map(|(i, content)| PlanningFact {
                key: key(i),
                generation: 1,
                content_identity: format!("{:x}", Sha256::digest(content.as_bytes())),
            })
            .collect()
    }
    fn ordinary(&self, remote: bool) -> Result<Plan, PlannerError> {
        let placements = fixture::placements(&self.fixture, remote);
        let hosts = &self.fixture.hosts[..if remote && self.remote_present { 2 } else { 1 }];
        let lines = if remote && self.line_present {
            std::slice::from_ref(&self.fixture.line)
        } else {
            &[]
        };
        let plan = plan_with_line_offers(
            &self.fixture.plot,
            hosts,
            &placements,
            &["conduit.base/local@1".into()],
            1,
            1,
            lines,
        )?;
        assert!(verify_plan(&plan));
        Ok(plan)
    }
}
fn key(i: usize) -> PlanningFactKey {
    PlanningFactKey::exact(
        if i == 2 {
            FactDomain::Line
        } else {
            FactDomain::Host
        },
        format!("line-oracle/{i}"),
    )
}
fn candidates(facts: &Facts) -> Vec<CandidateStructure> {
    [false, true]
        .into_iter()
        .map(|remote| CandidateStructure {
            candidate_id: remote.to_string(),
            semantic_contract_id: facts.fixture.plot.checked_plot_id.as_str().to_owned(),
            implementation_family_id: "math/chain".into(),
            placement_id: format!("placement/{remote}"),
            dependencies: if remote {
                vec![key(0), key(1), key(2)]
            } else {
                vec![key(0)]
            },
        })
        .collect()
}
fn compare(
    planner: &mut IncrementalPlanner,
    facts: &Facts,
    retained: &mut BTreeMap<String, Plan>,
) -> IncrementalPlan {
    let basis = facts.basis();
    let candidates = candidates(facts);
    let evaluate = |candidate: &CandidateStructure, fresh: &[PlanningFact]| {
        for fact in fresh {
            assert!(basis.contains(fact));
        }
        let remote = candidate.candidate_id == "true";
        let (disposition, result_identity) = match facts.ordinary(remote) {
            Ok(plan) => {
                let id = plan.plan_id.as_str().to_owned();
                if let Some(old) = retained.insert(id.clone(), plan.clone()) {
                    assert_eq!(old, plan);
                }
                (CandidateEvaluationDisposition::Admitted, id)
            }
            Err(error) => {
                let reason = format!("{error:?}");
                (
                    CandidateEvaluationDisposition::Rejected(reason.clone()),
                    reason,
                )
            }
        };
        CandidateEvaluation {
            disposition,
            result_identity,
            total_cost: if remote { 1 } else { 10 },
            evaluation_work_units: 1,
        }
    };
    let mut evaluate = evaluate;
    let incremental = planner
        .plan(
            &candidates,
            &basis,
            &StabilityPolicy::disabled(),
            &mut evaluate,
        )
        .unwrap();
    let cold = plan_cold(&candidates, &basis, &StabilityPolicy::disabled(), evaluate).unwrap();
    assert_eq!(
        incremental.selected_result_identity,
        cold.selected_result_identity
    );
    assert_eq!(incremental.selected_cost, cold.selected_cost);
    assert_eq!(incremental.considered.len(), cold.considered.len());
    for (cached, fresh) in incremental.considered.iter().zip(&cold.considered) {
        assert_eq!(cached.evaluation, fresh.evaluation);
    }
    assert!(verify_plan(
        &retained[&incremental.selected_result_identity]
    ));
    incremental
}

#[test]
fn exact_line_replacement_reseals_plan_without_changing_source_or_resource_contracts() {
    let mut facts = Facts::new();
    let mut planner = IncrementalPlanner::new(2).unwrap();
    let mut retained = BTreeMap::new();
    assert!(facts.ordinary(false).is_ok(), "{:?}", facts.ordinary(false));
    assert!(facts.ordinary(true).is_ok(), "{:?}", facts.ordinary(true));
    let first = compare(&mut planner, &facts, &mut retained);
    assert_eq!(first.selected_candidate_id, "true");
    let original = retained[&first.selected_result_identity].clone();
    let local = facts.ordinary(false).unwrap();
    facts.fixture.line.line_id = "replacement/line".into();
    facts.fixture.line.availability.line_id = facts.fixture.line.line_id.clone();
    facts.fixture.line.binding.binding_id = "replacement/binding".into();
    facts.fixture.line.availability.binding_id = facts.fixture.line.binding.binding_id.clone();
    facts.fixture.line.availability.sign_id = "replacement/ready".into();
    let next = compare(&mut planner, &facts, &mut retained);
    assert_eq!(next.metrics.invalidated_candidates, 1);
    assert_eq!(next.metrics.reused_candidates, 1);
    assert_ne!(
        first.selected_result_identity,
        next.selected_result_identity
    );
    let replacement = &retained[&next.selected_result_identity];
    assert_eq!(replacement.source_document_id, original.source_document_id);
    assert_eq!(replacement.checked_plot_id, original.checked_plot_id);
    assert_eq!(replacement.expanded_plot_id, original.expanded_plot_id);
    for (old, new) in original.fragments.iter().zip(&replacement.fragments) {
        assert_eq!(old.placements, new.placements);
        assert_ne!(old.fragment_id, new.fragment_id);
    }
    assert_eq!(retained[&first.selected_result_identity], original);
    assert_eq!(retained[local.plan_id.as_str()], local);
    assert!(verify_plan(&original));
    println!(
        "{}",
        serde_json::json!({
            "case": "Line-replacement", "original": original,
            "replacement": replacement, "independent": local,
            "selected_line": facts.fixture.line,
        })
    );
}

#[test]
fn missing_unavailable_stale_underbounded_lines_and_provider_loss_preserve_local_real_work() {
    for mutation in 0..5 {
        let mut facts = Facts::new();
        let mut planner = IncrementalPlanner::new(2).unwrap();
        let mut retained = BTreeMap::new();
        let first = compare(&mut planner, &facts, &mut retained);
        let original = retained[&first.selected_result_identity].clone();
        let local = facts.ordinary(false).unwrap();
        match mutation {
            0 => facts.line_present = false,
            1 => facts.fixture.line.availability.availability = LineAvailability::Unavailable,
            2 => facts.fixture.line.binding.sink.boot_id = "stale/boot".into(),
            3 => facts.fixture.line.binding.limits.maximum_payload_bytes = 0,
            4 => facts.remote_present = false,
            _ => unreachable!(),
        }
        let next = compare(&mut planner, &facts, &mut retained);
        assert_eq!(
            next.selected_result_identity,
            local.plan_id.as_str(),
            "mutation {mutation}"
        );
        assert_eq!(next.metrics.reused_candidates, 1);
        assert_eq!(next.metrics.invalidated_candidates, 1);
        assert!(matches!(
            next.considered[1].evaluation.disposition,
            CandidateEvaluationDisposition::Rejected(_)
        ));
        assert_eq!(retained[&first.selected_result_identity], original);
        assert_eq!(retained[local.plan_id.as_str()], local);
        assert!(verify_plan(&original) && verify_plan(&local));
        println!(
            "{}",
            serde_json::json!({
                "case": (["missing-line", "unavailable-line", "stale-peer-boot", "underbounded-line", "provider-loss"][mutation]),
                "original": original, "surviving": local,
                "refusal": next.considered[1].evaluation.result_identity,
                "invalidated_candidates": next.metrics.invalidated_candidates,
                "reused_candidates": next.metrics.reused_candidates,
            })
        );
        let stable = compare(&mut planner, &facts, &mut retained);
        assert_eq!(stable.metrics.reused_candidates, 2);
        facts = Facts::new();
        let restored = compare(&mut planner, &facts, &mut retained);
        assert_eq!(
            restored.selected_result_identity,
            first.selected_result_identity
        );
        assert_eq!(restored.metrics.reused_candidates, 1);
        assert_eq!(restored.metrics.invalidated_candidates, 1);
    }
}
