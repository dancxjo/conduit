use conduit_body::*;
use conduit_core::{BootId, CheckedFormId, PlanId, SignId, SourceDocumentId};

fn body_id(name: &str) -> BodyId {
    Body::born(
        SourceDocumentId::new(name),
        CheckedFormId::new("checked"),
        1,
        SignId::new(format!("born-{name}")),
    )
    .unwrap()
    .body_id
}
fn part(body: &BodyId, name: &str) -> PartId {
    PartId::bind(body, name, 1).unwrap()
}
fn claim(body: &BodyId, name: &str, generation: u64) -> PartContinuityClaim {
    PartContinuityClaim {
        body_id: body.clone(),
        part_id: part(body, name),
        membership_generation: generation,
        continuity_proof: format!("proof-{name}-{generation}"),
        workload_revision: 1,
    }
}
fn runtime(body: &BodyId, name: &str, boot: &str) -> CurrentPartRuntime {
    CurrentPartRuntime {
        part_id: part(body, name),
        boot_id: BootId::new(boot),
        plan_id: Some(PlanId::new("old-plan")),
    }
}

#[test]
fn body_survives_attrition_reboot_and_replacement_until_final_part_loss() {
    let body = body_id("body");
    let mut continuity =
        DurableBodyContinuity::establish(claim(&body, "a", 1), runtime(&body, "a", "boot-a"), 4)
            .unwrap();
    continuity
        .admit(claim(&body, "b", 1), runtime(&body, "b", "boot-b"))
        .unwrap();
    continuity
        .admit(claim(&body, "c", 1), runtime(&body, "c", "boot-c"))
        .unwrap();
    continuity.lose_part(&part(&body, "a")).unwrap();
    continuity
        .reboot(&part(&body, "b"), BootId::new("boot-b2"))
        .unwrap();
    continuity.lose_part(&part(&body, "c")).unwrap();
    assert!(continuity.is_continuing());
    assert_eq!(continuity.body_id, body);
    continuity
        .admit(
            claim(&body, "replacement", 1),
            runtime(&body, "replacement", "fresh"),
        )
        .unwrap();
    assert_eq!(continuity.current_parts(), 2);
    continuity.lose_part(&part(&body, "b")).unwrap();
    continuity.lose_part(&part(&body, "replacement")).unwrap();
    assert!(!continuity.is_continuing());
    assert_eq!(
        continuity.admit(
            claim(&body, "fabricated", 1),
            runtime(&body, "fabricated", "boot")
        ),
        Err(BodyContinuityRefusal::BodyExtinct)
    );
}

#[test]
fn stale_and_unrelated_claimants_refuse() {
    let body = body_id("body");
    let other = body_id("other");
    let mut continuity =
        DurableBodyContinuity::establish(claim(&body, "a", 2), runtime(&body, "a", "boot"), 2)
            .unwrap();
    assert_eq!(
        continuity.admit(claim(&body, "a", 1), runtime(&body, "a", "stale")),
        Err(BodyContinuityRefusal::StaleClaim)
    );
    assert_eq!(
        continuity.admit(claim(&other, "x", 1), runtime(&other, "x", "boot")),
        Err(BodyContinuityRefusal::InvalidProof)
    );
}

#[test]
fn workload_revision_commits_atomically_or_preserves_previous_truth() {
    let mut transition = WorkloadTransition::propose(4, 5).unwrap();
    transition.check().unwrap();
    transition.plan("plan-5".into()).unwrap();
    transition.reserve().unwrap();
    transition.prepare().unwrap();
    transition.commit(4, Some("play-5".into())).unwrap();
    assert!(matches!(
        transition.state,
        WorkloadTransitionState::Committed { revision: 5, .. }
    ));
    let mut competing = WorkloadTransition::propose(4, 5).unwrap();
    competing.check().unwrap();
    competing.plan("plan-x".into()).unwrap();
    competing.reserve().unwrap();
    competing.prepare().unwrap();
    assert_eq!(
        competing.commit(5, None),
        Err(WorkloadTransitionRefusal::ConcurrentEdit)
    );
    assert!(matches!(
        competing.state,
        WorkloadTransitionState::Refused {
            previous_revision: 4,
            ..
        }
    ));
}

#[test]
fn administration_checks_authority_staleness_and_replay() {
    let request = BodyAdministrativeRequest {
        request_id: "r1".into(),
        subject: "body".into(),
        authority: Some("grant".into()),
        expected_revision: 2,
        intent: BodyAdministrativeIntent::AddForm,
    };
    assert_eq!(administer(&request, 2, &[]).unwrap().resulting_revision, 3);
    assert_eq!(
        administer(&request, 2, &["r1".into()]),
        Err(BodyAdministrativeRefusal::Replay)
    );
    let mut denied = request.clone();
    denied.request_id = "r2".into();
    denied.authority = None;
    assert_eq!(
        administer(&denied, 2, &[]),
        Err(BodyAdministrativeRefusal::Denied)
    );
    assert_eq!(
        administer(&request, 3, &[]),
        Err(BodyAdministrativeRefusal::Stale)
    );
}
