//! Deterministic Host observations; no DOM acquisition or physical claim.
use super::*;

pub(in crate::form_runner) fn request() -> BodyStartRequest {
    let plans = ["first", "second"].into_iter().map(|name| {
        let source = format!("form {name} {{\n text: text/literal(\"{name}\")\n show: presentation/text\n text > show\n}}\n");
        let (session, _) = TourSession::prepare("body-host", "body-boot", &source, 1).unwrap();
        let fragment = session.fragments[0].clone();
        conduit_core::seal_plan(conduit_core::FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        }, vec![fragment])
    }).collect::<Vec<_>>();
    let body = conduit_body::Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_form_id.clone(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let body = body
        .admit_form(
            conduit_body::ResidentForm::new(
                plans[1].source_document_id.clone(),
                plans[1].checked_form_id.clone(),
            ),
            "sign/admit".into(),
        )
        .unwrap();
    let wake = body.wake(1, "sign/wake".into()).unwrap().1;
    let plan = BodyPlan::seal(
        &wake,
        plans
            .into_iter()
            .map(|plan| conduit_body::BodyFormPlan {
                form: conduit_body::ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .unwrap();
    let host = crate::installed_browser::advertisement("body-host".into(), "body-boot".into());
    let observations = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: conduit_core::ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: format!("sign/observed-{index}").into(),
        })
        .collect();
    BodyStartRequest {
        wake,
        plan,
        play_sequence: 7,
        observations,
    }
}

#[test]
fn exact_body_starts_one_existing_browser_session_and_preserves_partition_identity() {
    let request = request();
    let original = request.plan.clone();
    let (mut session, started) = prepare(request).unwrap();
    assert!(started.play.validate_for(&original));
    assert_eq!(session.active_play_id, started.play.active_play_id);
    assert_eq!(
        session
            ._resource_admissions
            .as_ref()
            .unwrap()
            .admissions()
            .len(),
        2
    );
    assert_eq!(
        started.wake_at_start.lifecycle,
        conduit_body::WakeLifecycle::Playing
    );
    let mut progress = started.progress;
    let mut seen = Vec::new();
    loop {
        match progress {
            TourProgress::Effect(effect) => {
                let TourHostEffect::Manifestation(effect) = *effect else {
                    panic!("unexpected effect")
                };
                assert_eq!(effect.active_play_id, started.play.active_play_id.as_str());
                assert!(original
                    .forms
                    .iter()
                    .any(|part| part.plan.plan_id.as_str() == effect.plan_id));
                seen.push(effect.plan_id);
                progress = session.advance().unwrap();
            }
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.active_play_id, started.play.active_play_id.as_str());
                let sign = bind_sign(
                    &session.host_id,
                    &session.boot_id,
                    Some(&session.active_play_id),
                    2,
                );
                assert_eq!(receipt.terminal_sign_id, sign.sign_id.as_str());
                break;
            }
            _ => panic!("unexpected progress"),
        }
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        session
            .fragments
            .iter()
            .map(|part| &part.plan_id)
            .collect::<Vec<_>>(),
        original
            .forms
            .iter()
            .map(|part| &part.plan.plan_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn missing_and_stale_observations_or_workload_refuse_before_body_start() {
    let mut missing = request();
    missing.observations.clear();
    assert!(prepare(missing)
        .err()
        .unwrap()
        .contains("MissingObservation"));
    let mut stale = request();
    for observation in &mut stale.observations {
        observation.boot_id = "stale".into();
    }
    assert!(prepare(stale).is_err());
    let mut changed = request();
    changed.plan.workload_revision += 1;
    assert!(prepare(changed).err().unwrap().contains("StaleWorkload"));
}
