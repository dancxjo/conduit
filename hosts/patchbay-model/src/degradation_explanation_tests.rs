use conduit_core::PlanId;
use conduit_planner::{
    DegradationAssessment, DegradationFragment, DegradationFragmentDisposition, FactDomain,
    PlanningFactKey,
};

use crate::PatchbayDegradationExplanation;

fn assessment(replacement: bool) -> DegradationAssessment {
    DegradationAssessment {
        previous_plan_id: PlanId::from("plan/heterogeneous/old"),
        replacement_plan_id: replacement.then(|| PlanId::from("plan/heterogeneous/fresh")),
        fragments: vec![
            DegradationFragment {
                fragment_id: "fragment/edge-input-filter".into(),
                previous_candidate_id: "edge-primary".into(),
                changed_dependencies: vec![],
                disposition: DegradationFragmentDisposition::StillWorks,
                reused_unaffected_structure: true,
            },
            DegradationFragment {
                fragment_id: "fragment/heavy-compute".into(),
                previous_candidate_id: "compute-gpu".into(),
                changed_dependencies: vec![PlanningFactKey::exact(
                    FactDomain::Resource,
                    "resource/compute-gpu",
                )],
                disposition: if replacement {
                    DegradationFragmentDisposition::Replaced {
                        candidate_id: "compute-spare-cpu".into(),
                    }
                } else {
                    DegradationFragmentDisposition::Refused {
                        reason: "no current host offers the required compute capacity".into(),
                    }
                },
                reused_unaffected_structure: false,
            },
        ],
        automatic_retry_count: 0,
    }
}

#[test]
fn patchbay_names_failure_continuing_work_and_distinct_fresh_plan() {
    let explanation = PatchbayDegradationExplanation::from_assessment(&assessment(true)).unwrap();
    assert_eq!(explanation.what_failed.len(), 1);
    assert!(explanation.what_failed[0].contains("resource/compute-gpu"));
    assert!(explanation.what_failed[0].contains("replacement=compute-spare-cpu"));
    assert_eq!(explanation.what_still_works.len(), 1);
    assert!(explanation.what_still_works[0].contains("unaffected structure reused=true"));
    assert!(explanation
        .what_changed
        .contains("historical Plan plan/heterogeneous/old remains immutable"));
    assert!(explanation
        .what_changed
        .contains("fresh ordinary Plan=plan/heterogeneous/fresh"));
    assert_eq!(explanation.automatic_retry_count, 0);
}

#[test]
fn patchbay_keeps_specific_refusal_distinct_from_body_wide_failure() {
    let explanation = PatchbayDegradationExplanation::from_assessment(&assessment(false)).unwrap();
    assert!(
        explanation.what_failed[0].contains("no current host offers the required compute capacity")
    );
    assert!(explanation
        .what_changed
        .contains("no complete replacement Plan exists"));
    assert!(explanation.what_still_works[0].contains("edge-input-filter"));
}

#[test]
fn patchbay_refuses_any_claim_of_automatic_retry() {
    let mut evidence = assessment(true);
    evidence.automatic_retry_count = 1;
    assert!(PatchbayDegradationExplanation::from_assessment(&evidence).is_err());
}
