//! Deterministic Host observations; no DOM acquisition or physical claim.
use super::*;

#[test]
fn living_button_keeps_the_completed_clock_and_telegraph_in_one_body_play() {
    let request = request_from_sources(&[
        include_str!("../../../../../forms/button-across-room/main.conduit"),
        include_str!("../../../../../forms/clock/main.conduit"),
        include_str!("../../../../../forms/desk-telegraph/main.conduit"),
    ]);
    let original = request.plan.clone();
    let (mut session, started) = prepare(request).unwrap();
    assert!(started.play.validate_for(&original));
    let mut progress = started.progress;
    let mut transitions = 0_u64;
    let mut ticks = Vec::new();
    let mut levels = Vec::new();
    let mut telegraph = false;
    let mut keys = 0_usize;
    loop {
        match progress {
            TourProgress::Effect(effect) => {
                let output = match *effect {
                    TourHostEffect::ButtonTransition(_) => {
                        let bytes = conduit_semantic_catalog::button_transition_value(
                            "button/primary",
                            transitions.is_multiple_of(2),
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
                    TourHostEffect::KeyEvent(_) => {
                        let (usage, transition) = match keys {
                            0 => (0x04, conduit_human::KeyTransition::Pressed),
                            1 => (0x04, conduit_human::KeyTransition::Released),
                            2 => (0x28, conduit_human::KeyTransition::Pressed),
                            3 => (0x28, conduit_human::KeyTransition::Released),
                            extra if extra.is_multiple_of(2) => {
                                (0x04, conduit_human::KeyTransition::Pressed)
                            }
                            _ => (0x04, conduit_human::KeyTransition::Released),
                        };
                        keys += 1;
                        Some(
                            conduit_human::KeyEvent::new(
                                usage,
                                transition,
                                conduit_human::KeyModifiers::NONE,
                            )
                            .unwrap()
                            .encode()
                            .to_vec(),
                        )
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
                                assert_eq!(value.text.as_deref(), Some("a"));
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
                if transitions >= 2 && ticks.len() >= 5 && levels.len() >= 2 && telegraph {
                    break;
                }
            }
            TourProgress::Waiting {
                disposition,
                active_play_id,
                pending_effects,
                ..
            } => {
                assert_eq!(disposition, "quiescent_awaiting_input");
                assert_eq!(active_play_id, started.play.active_play_id.as_str());
                assert_eq!(pending_effects, 0);
                panic!("standing Body became quiescent before its acceptance observations");
            }
            _ => panic!("unexpected progress"),
        }
    }
    assert!(transitions >= 2);
    assert!(ticks.len() >= 5);
    assert!(ticks
        .iter()
        .enumerate()
        .all(|(index, tick)| tick == &index.to_string()));
    assert!(levels.len() >= 2);
    assert!(levels.iter().enumerate().all(|(index, level)| level
        == if index.is_multiple_of(2) {
            "true"
        } else {
            "false"
        }));
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
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn canonical_firefly_and_unrelated_text_remain_in_one_body_play_until_stop() {
    let request = request_from_sources(&[
        include_str!("../../../../../forms/firefly-choir/main.conduit"),
        "form unrelated {\n complete\n message: text/literal(\"unrelated workload\")\n show: presentation/text\n message > show\n}\n",
    ]);
    let original = request.plan.clone();
    let (mut session, started) = prepare(request).unwrap();
    assert!(started.play.validate_for(&original));
    assert_eq!(original.forms.len(), 2);

    let mut progress = started.progress;
    let mut pulses = Vec::new();
    let mut rhythms = Vec::new();
    let mut unrelated = false;
    let receipt = loop {
        match progress {
            TourProgress::Effect(effect) => {
                match *effect {
                    TourHostEffect::Timer(timer) => assert_eq!(timer.duration_millis, 240),
                    TourHostEffect::Manifestation(value) => {
                        assert_eq!(value.active_play_id, started.play.active_play_id.as_str());
                        assert!(original
                            .forms
                            .iter()
                            .any(|part| part.plan.plan_id.as_str() == value.plan_id));
                        match value.presentation_kind.as_str() {
                            conduit_semantic_catalog::PULSE_PRESENTATION_KIND => {
                                pulses.push(value.text.unwrap())
                            }
                            conduit_semantic_catalog::RHYTHM_PRESENTATION_KIND => {
                                rhythms.push(value.text.unwrap())
                            }
                            "presentation/text" => {
                                assert_eq!(value.text.as_deref(), Some("unrelated workload"));
                                unrelated = true;
                            }
                            _ => panic!("unexpected manifestation"),
                        }
                    }
                    _ => panic!("unexpected effect"),
                }
                progress = session.advance().unwrap();
                if pulses.len() >= 5 && rhythms.len() >= 5 && unrelated {
                    assert_eq!(session.fragments.len(), 2);
                    break session.cancel().unwrap();
                }
            }
            TourProgress::Receipt(receipt) => {
                panic!("standing Firefly ended before Stop: {receipt:?}");
            }
            _ => panic!("unexpected progress"),
        }
    };

    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.active_play_id, started.play.active_play_id.as_str());
    assert_eq!(pulses.len(), 5);
    assert_eq!(rhythms.len(), 5);
    assert!(unrelated);
}

#[test]
fn canonical_signal_garden_and_unrelated_text_complete_in_one_body_play() {
    let request = request_from_sources(&[
        include_str!("../../../../../forms/signal-garden/main.conduit"),
        "form unrelated {\n complete\n message: text/literal(\"unrelated workload\")\n show: presentation/text\n message > show\n}\n",
    ]);
    let original = request.plan.clone();
    let (mut session, started) = prepare(request).unwrap();
    assert!(started.play.validate_for(&original));
    assert_eq!(original.forms.len(), 2);

    let mut progress = started.progress;
    let mut garden = false;
    let mut unrelated = false;
    loop {
        match progress {
            TourProgress::Effect(effect) => {
                let TourHostEffect::Manifestation(value) = *effect else {
                    panic!("deterministic Garden source must not request a Host effect")
                };
                assert_eq!(value.active_play_id, started.play.active_play_id.as_str());
                assert!(original
                    .forms
                    .iter()
                    .any(|part| part.plan.plan_id.as_str() == value.plan_id));
                match value.presentation_kind.as_str() {
                    conduit_semantic_catalog::GARDEN_STATE_PRESENTATION_KIND => {
                        assert_eq!(
                            value.text.as_deref(),
                            Some("garden step 1 · vitality 0.600000 · activity 0.400000")
                        );
                        garden = true;
                    }
                    "presentation/text" => {
                        assert_eq!(value.text.as_deref(), Some("unrelated workload"));
                        unrelated = true;
                    }
                    _ => panic!("unexpected manifestation"),
                }
                progress = session.advance().unwrap();
            }
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                assert_eq!(receipt.active_play_id, started.play.active_play_id.as_str());
                assert_eq!(receipt.manifestation_completions, 2);
                break;
            }
            _ => panic!("unexpected progress"),
        }
    }

    assert!(garden);
    assert!(unrelated);
    assert_eq!(session.fragments.len(), 2);
}

#[test]
fn unchanged_canonical_clock_uses_installed_browser_tick_presentation() {
    let source = include_str!("../../../../../forms/clock/main.conduit");
    let (mut session, effect) =
        TourSession::prepare("clock-browser", "clock-boot", source, 1).unwrap();
    let mut progress = TourProgress::Effect(Box::new(effect));
    let mut ticks = Vec::new();
    let mut waits = 0;
    let receipt = loop {
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
                if ticks.len() == 5 {
                    break session.cancel().unwrap();
                }
            }
            TourProgress::Receipt(receipt) => {
                panic!("standing clock ended before Stop: {receipt:?}");
            }
            _ => panic!("unexpected clock progress"),
        }
    };
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(waits, 5);
    assert_eq!(ticks, ["0", "1", "2", "3", "4"]);
    let malformed = crate::installed_browser::BrowserManifestation {
        kind_id: conduit_semantic_catalog::TICK_PRESENTATION_KIND,
        canonical_value: vec![0; 7],
    };
    assert!(decode_manifestation(&malformed).is_err());
}

pub(in crate::form_runner) fn request() -> BodyStartRequest {
    request_from_sources(&[
        "form first {\n complete\n text: text/literal(\"first\")\n show: presentation/text\n text > show\n}\n",
        "form second {\n complete\n text: text/literal(\"second\")\n show: presentation/text\n text > show\n}\n",
    ])
}

pub(super) fn request_from_sources(sources: &[&str]) -> BodyStartRequest {
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
            let entry = super::executable_entry(&checked).unwrap();
            let form = conduit_form::expand_canonical_form(&checked, &entry, &catalog).unwrap();
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
