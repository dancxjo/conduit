//! Deterministic Host observations; no DOM acquisition or physical claim.
use super::*;

#[test]
fn canonical_button_clock_and_telegraph_complete_in_one_body_play() {
    let request = request_from_sources(&[
        include_str!("../../../../../forms/button-across-room/main.conduit"),
        include_str!("../../../../../forms/clock/main.conduit"),
        include_str!("../../../../../forms/desk-telegraph/main.conduit"),
    ]);
    let original = request.plan.clone();
    let (mut session, started) = prepare(request).unwrap();
    assert!(started.play.validate_for(&original));
    let mut progress = started.progress;
    let mut transitions = 0;
    let mut ticks = Vec::new();
    let mut levels = Vec::new();
    let mut telegraph = false;
    loop {
        match progress {
            TourProgress::Effect(effect) => {
                let output = match *effect {
                    TourHostEffect::ButtonTransition(_) => {
                        let bytes = conduit_semantic_catalog::button_transition_value(
                            "button/primary",
                            transitions == 0,
                            transitions,
                        )
                        .unwrap()
                        .canonical_bytes()
                        .unwrap();
                        transitions += 1;
                        Some(bytes)
                    }
                    TourHostEffect::Timer(timer) => {
                        assert_eq!(timer.duration_millis, 1000);
                        None
                    }
                    TourHostEffect::Manifestation(value) => {
                        assert_eq!(value.active_play_id, started.play.active_play_id.as_str());
                        assert!(original
                            .forms
                            .iter()
                            .any(|part| part.plan.plan_id.as_str() == value.plan_id));
                        match value.presentation_kind.as_str() {
                            conduit_semantic_catalog::TICK_PRESENTATION_KIND => {
                                ticks.push(value.text.unwrap())
                            }
                            conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_KIND => {
                                levels.push(value.text.unwrap())
                            }
                            "presentation/text" => {
                                assert_eq!(value.text.as_deref(), Some("CALLING"));
                                telegraph = true;
                            }
                            _ => panic!("unexpected manifestation"),
                        }
                        None
                    }
                    _ => panic!("unexpected effect"),
                };
                progress = match output {
                    Some(bytes) => session.advance_with_output(&bytes).unwrap(),
                    None => session.advance().unwrap(),
                };
            }
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                break;
            }
            _ => panic!("unexpected progress"),
        }
    }
    assert_eq!(transitions, 2);
    assert_eq!(ticks, ["0", "1", "2", "3"]);
    assert_eq!(levels, ["true", "false"]);
    assert!(telegraph);
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
fn unchanged_canonical_clock_uses_installed_browser_tick_presentation() {
    let source = include_str!("../../../../../forms/clock/main.conduit");
    let (mut session, effect) =
        TourSession::prepare("clock-browser", "clock-boot", source, 1).unwrap();
    let mut progress = TourProgress::Effect(Box::new(effect));
    let mut ticks = Vec::new();
    let mut waits = 0;
    loop {
        match progress {
            TourProgress::Effect(effect) => {
                match *effect {
                    TourHostEffect::Timer(timer) => {
                        assert_eq!(timer.duration_millis, 1000);
                        waits += 1;
                    }
                    TourHostEffect::Manifestation(value) => {
                        assert_eq!(
                            value.presentation_kind,
                            conduit_semantic_catalog::TICK_PRESENTATION_KIND
                        );
                        ticks.push(value.text.unwrap());
                    }
                    _ => panic!("unexpected clock effect"),
                }
                progress = session.advance().unwrap();
            }
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                break;
            }
            _ => panic!("unexpected clock progress"),
        }
    }
    assert_eq!(waits, conduit_time::TIME_EVERY_COUNT);
    assert_eq!(ticks, ["0", "1", "2", "3"]);
    let malformed = crate::installed_browser::BrowserManifestation {
        kind_id: conduit_semantic_catalog::TICK_PRESENTATION_KIND,
        canonical_value: vec![0; 7],
    };
    assert!(decode_manifestation(&malformed).is_err());
}

pub(in crate::form_runner) fn request() -> BodyStartRequest {
    request_from_sources(&[
        "form first {\n text: text/literal(\"first\")\n show: presentation/text\n text > show\n}\n",
        "form second {\n text: text/literal(\"second\")\n show: presentation/text\n text > show\n}\n",
    ])
}

fn request_from_sources(sources: &[&str]) -> BodyStartRequest {
    let (startup, catalog) = crate::installed_browser::catalogs().unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "body-host".into(),
        "body-boot".into(),
    )];
    let plans = sources
        .iter()
        .map(|source| {
            let checked = conduit_form::check_syntax_document(
                &conduit_form::parse_syntax_document(source),
                &startup,
            )
            .unwrap();
            let form = conduit_form::expand_canonical_form(
                &checked,
                &checked.forms.last().unwrap().name,
                &catalog,
            )
            .unwrap();
            let placements = default_expanded_placements(&form, &hosts).unwrap();
            plan_expanded_canonical_with_options(
                &form,
                &hosts,
                &placements,
                &local_bases(),
                PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES
                        as u32,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let body = conduit_body::Body::born(
        plans[0].source_document_id.clone(),
        plans[0].checked_form_id.clone(),
        1,
        "sign/born".into(),
    )
    .unwrap();
    let mut body = body;
    for (index, plan) in plans.iter().enumerate().skip(1) {
        body = body
            .admit_form(
                conduit_body::ResidentForm::new(
                    plan.source_document_id.clone(),
                    plan.checked_form_id.clone(),
                ),
                format!("sign/admit-{index}").into(),
            )
            .unwrap();
    }
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
