use conduit_body::{
    AuthenticatedHostObservation, BodyMembership, HostPresenceClock, HostPresenceClockScale,
    HostPresenceTable, MembershipProofId, PartId, WakeLifecycle,
};
use conduit_core::{
    BootId, ControlLoopEvent, GearId, HostId, LineAvailability, LineAvailabilitySign,
    LinkBindingId, OfferGeneration, PlanningRefusalReason, PlayUnsatisfiedReason, SignId,
};
use conduit_system_continuity::{
    exact_r1_signal_plan, R1LedResultObservation, R1NewPlanRecovery, R1RecoveryError,
    R1RecoveryStartSigns, R1ReplacementSigns, R1SignalRouteSet, MAX_R1_RECOVERY_EVENTS,
};

fn start() -> (R1NewPlanRecovery, conduit_core::Plan, conduit_core::Plan) {
    let boot = BootId::from("r1/pico-runtime-boot");
    let plan_a = exact_r1_signal_plan(boot.clone(), R1SignalRouteSet::WebSocketOnly)
        .unwrap()
        .plan;
    let plan_b = exact_r1_signal_plan(boot, R1SignalRouteSet::UsbOnly)
        .unwrap()
        .plan;
    let recovery = R1NewPlanRecovery::begin(
        plan_a.clone(),
        GearId::from("signal-demo/show"),
        1,
        1,
        HostId::from(conduit_net::R1_STD_HOST_ID),
        BootId::from(conduit_net::R1_STD_BOOT_ID),
        0,
        R1RecoveryStartSigns {
            birth: SignId::from("r1/body-born"),
            wake: SignId::from("r1/body-woke"),
            plan_ready: SignId::from("r1/plan-a-ready"),
            play_started: SignId::from("r1/play-a-started"),
        },
    )
    .unwrap();
    (recovery, plan_a, plan_b)
}

fn lose_websocket(recovery: &mut R1NewPlanRecovery) {
    recovery
        .observe_line_unavailable(
            LineAvailabilitySign {
                line_id: conduit_core::LineId::from(conduit_net::R1_WEBSOCKET_LINE_ID),
                binding_id: conduit_core::LinkBindingId::from(
                    conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
                ),
                availability: LineAvailability::Unavailable,
                sign_id: SignId::from("r1/websocket-unavailable"),
            },
            SignId::from("r1/play-a-unsatisfied"),
        )
        .unwrap();
}

fn presence_for(
    recovery: &R1NewPlanRecovery,
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    lose_session: bool,
) -> (HostPresenceTable, PartId) {
    let body_id = recovery.body().body_id.clone();
    let part_id = PartId::bind(&body_id, host_id.as_str(), 1).unwrap();
    let mut membership = BodyMembership::new(body_id.clone()).unwrap();
    let proof_id = MembershipProofId::bind("r1/host-presence-proof").unwrap();
    membership
        .admit(
            &body_id,
            membership.revision,
            part_id.clone(),
            proof_id.clone(),
            SignId::from("r1/host-part-admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id,
                boot_id,
                offer_generation,
                proof_id,
                sequence: 1,
            },
            SignId::from("r1/host-attached"),
        )
        .unwrap();
    let session = LinkBindingId::from("r1/host-presence-session");
    let clock = HostPresenceClock::new(
        "clock/r1-recovery/conformance".into(),
        HostPresenceClockScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    let mut presence = HostPresenceTable::new(body_id, clock, 30_000).unwrap();
    presence
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            1_000,
            20_000,
            SignId::from("r1/host-presence-started"),
        )
        .unwrap();
    if lose_session {
        presence
            .lose_session(
                &mut membership,
                &part_id,
                &session,
                2_000,
                SignId::from("r1/host-session-lost"),
            )
            .unwrap();
    }
    (presence, part_id)
}

#[test]
fn exact_selected_host_loss_makes_play_unsatisfied_without_mutating_plan() {
    let (mut recovery, plan_a, _) = start();
    let selected = plan_a
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
        .unwrap();
    let (presence, part_id) = presence_for(
        &recovery,
        selected.host_id.clone(),
        selected.boot_id.clone(),
        selected.offer_generation,
        true,
    );
    let body_id = recovery.body().body_id.clone();
    let wake_id = recovery.wake().wake_id.clone();

    recovery
        .observe_required_host_unavailable(
            &presence,
            &part_id,
            SignId::from("r1/required-host-unsatisfied"),
        )
        .unwrap();

    assert_eq!(recovery.body().body_id, body_id);
    assert_eq!(recovery.wake().wake_id, wake_id);
    assert_eq!(recovery.wake().lifecycle, WakeLifecycle::Unsatisfied);
    assert_eq!(recovery.plan_a(), &plan_a);
    assert!(recovery.plan_b().is_none());
    assert!(matches!(
        &recovery.events()[0],
        ControlLoopEvent::HostBecameUnavailable {
            plan_id,
            host_id,
            boot_id,
            offer_generation,
            observation_sign_id,
        } if plan_id == &plan_a.plan_id
            && host_id == &selected.host_id
            && boot_id == &selected.boot_id
            && offer_generation == &selected.offer_generation
            && observation_sign_id.as_str() == "r1/host-session-lost"
    ));
    assert!(matches!(
        &recovery.events()[1],
        ControlLoopEvent::PlayBecameUnsatisfied {
            plan_id,
            reason: PlayUnsatisfiedReason::RequiredHostUnavailable,
            sign_id,
        } if plan_id == &plan_a.plan_id && sign_id.as_str() == "r1/required-host-unsatisfied"
    ));
}

#[test]
fn non_loss_and_non_selected_host_observations_refuse_without_mutation() {
    let (recovery, plan_a, _) = start();
    let selected = plan_a
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
        .unwrap();
    let (available, selected_part) = presence_for(
        &recovery,
        selected.host_id.clone(),
        selected.boot_id.clone(),
        selected.offer_generation,
        false,
    );
    let mut available_recovery = recovery.clone();
    assert_eq!(
        available_recovery.observe_required_host_unavailable(
            &available,
            &selected_part,
            SignId::from("r1/must-not-be-unsatisfied"),
        ),
        Err(R1RecoveryError::InvalidObservation)
    );
    assert_eq!(available_recovery, recovery);

    let (stale, stale_part) = presence_for(
        &recovery,
        selected.host_id.clone(),
        BootId::from("r1/stale-pico-boot"),
        selected.offer_generation,
        true,
    );
    let mut stale_recovery = recovery.clone();
    assert_eq!(
        stale_recovery.observe_required_host_unavailable(
            &stale,
            &stale_part,
            SignId::from("r1/stale-must-not-be-unsatisfied"),
        ),
        Err(R1RecoveryError::WrongRealizationSubject)
    );
    assert_eq!(stale_recovery, recovery);

    let (stale_offer, stale_offer_part) = presence_for(
        &recovery,
        selected.host_id.clone(),
        selected.boot_id.clone(),
        OfferGeneration(selected.offer_generation.0 + 1),
        true,
    );
    let mut stale_offer_recovery = recovery.clone();
    assert_eq!(
        stale_offer_recovery.observe_required_host_unavailable(
            &stale_offer,
            &stale_offer_part,
            SignId::from("r1/stale-offer-must-not-be-unsatisfied"),
        ),
        Err(R1RecoveryError::WrongRealizationSubject)
    );
    assert_eq!(stale_offer_recovery, recovery);

    let (unselected, unselected_part) = presence_for(
        &recovery,
        HostId::from("r1/unselected-host"),
        BootId::from("r1/unselected-boot"),
        OfferGeneration(1),
        true,
    );
    let mut unselected_recovery = recovery.clone();
    assert_eq!(
        unselected_recovery.observe_required_host_unavailable(
            &unselected,
            &unselected_part,
            SignId::from("r1/unselected-must-not-be-unsatisfied"),
        ),
        Err(R1RecoveryError::WrongRealizationSubject)
    );
    assert_eq!(unselected_recovery, recovery);

    let mut wrong_body = stale.clone();
    wrong_body.body_id = conduit_body::Body::born(
        conduit_core::SourceDocumentId::from("r1/other-source"),
        conduit_core::CheckedFormId::from("r1/other-checked"),
        99,
        SignId::from("r1/other-body-born"),
    )
    .unwrap()
    .body_id;
    let mut wrong_body_recovery = recovery.clone();
    assert_eq!(
        wrong_body_recovery.observe_required_host_unavailable(
            &wrong_body,
            &stale_part,
            SignId::from("r1/wrong-body-must-not-be-unsatisfied"),
        ),
        Err(R1RecoveryError::InvalidObservation)
    );
    assert_eq!(wrong_body_recovery, recovery);
}

#[test]
fn one_body_and_wake_replace_immutable_plan_and_play_after_websocket_exhaustion() {
    let (mut recovery, original_plan_a, plan_b) = start();
    let body_id = recovery.body().body_id.clone();
    let wake_id = recovery.wake().wake_id.clone();
    let play_a_id = recovery.play_a().active_play_id.clone();
    let plan_a_session = recovery.plan_a_session_binding().unwrap();
    assert_eq!(
        plan_a_session.attachment.base,
        conduit_core::ConnectionBase::WebSocket
    );
    lose_websocket(&mut recovery);
    recovery
        .install_replacement(
            plan_b.clone(),
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            1,
            R1ReplacementSigns {
                request: SignId::from("r1/replan-requested"),
                planned: SignId::from("r1/plan-b-planned"),
                superseded: SignId::from("r1/plan-a-superseded"),
                realized: SignId::from("r1/plan-b-realized"),
                play_started: SignId::from("r1/play-b-started"),
            },
        )
        .unwrap();

    assert_eq!(recovery.body().body_id, body_id);
    assert_eq!(recovery.wake().wake_id, wake_id);
    assert_eq!(recovery.plan_a(), &original_plan_a);
    assert_eq!(recovery.plan_b().unwrap(), &plan_b);
    assert_ne!(
        recovery.plan_a().plan_id,
        recovery.plan_b().unwrap().plan_id
    );
    assert_ne!(play_a_id, recovery.play_b().unwrap().active_play_id);
    let plan_b_session = recovery.plan_b_session_binding().unwrap();
    assert_eq!(
        plan_b_session.attachment.base,
        conduit_core::ConnectionBase::UsbCdc
    );
    assert_ne!(plan_a_session.plan_id, plan_b_session.plan_id);
    assert_eq!(plan_a_session.sink, plan_b_session.sink);
    assert_eq!(recovery.events().len(), 6);
    assert!(matches!(
        recovery.events()[0],
        ControlLoopEvent::LineBecameUnavailable { .. }
    ));
    assert!(matches!(
        recovery.events()[1],
        ControlLoopEvent::PlayBecameUnsatisfied { .. }
    ));
    assert!(matches!(
        recovery.wake().events.iter().find(|event| matches!(
            event,
            conduit_body::WakeLifecycleEvent::Replanned { .. }
        )),
        Some(conduit_body::WakeLifecycleEvent::Replanned { sign_id, .. })
            if sign_id.as_str() == "r1/plan-b-planned"
    ));

    let pico_host = HostId::from(conduit_net::R1_PICO_HOST_ID);
    let pico_boot = BootId::from("r1/pico-runtime-boot");
    let plan_b_session = recovery.plan_b_session_binding().unwrap();
    assert_eq!(
        recovery.record_led_result(R1LedResultObservation {
            pico_host_id: pico_host.clone(),
            pico_boot_id: pico_boot.clone(),
            plan_id: original_plan_a.plan_id.clone(),
            active_play_id: play_a_id,
            observed_session: plan_b_session.clone(),
            sign_id: SignId::from("r1/stale-led-result"),
            level: true,
        }),
        Err(R1RecoveryError::StaleResult)
    );
    recovery
        .record_led_result(R1LedResultObservation {
            pico_host_id: pico_host.clone(),
            pico_boot_id: pico_boot.clone(),
            plan_id: plan_b.plan_id.clone(),
            active_play_id: recovery.play_b().unwrap().active_play_id.clone(),
            observed_session: plan_b_session.clone(),
            sign_id: SignId::from("r1/plan-b-led-on"),
            level: true,
        })
        .unwrap();
    let stale_session = plan_b_session
        .clone()
        .with_observed_boots(
            plan_b_session.source.boot_id.clone(),
            BootId::from("r1/stale-pico-boot"),
        )
        .unwrap();
    assert_eq!(
        recovery.record_led_result(R1LedResultObservation {
            pico_host_id: pico_host,
            pico_boot_id: pico_boot,
            plan_id: plan_b.plan_id.clone(),
            active_play_id: recovery.play_b().unwrap().active_play_id.clone(),
            observed_session: stale_session,
            sign_id: SignId::from("r1/stale-observed-session"),
            level: true,
        }),
        Err(R1RecoveryError::StaleResult)
    );
    assert_eq!(recovery.led_results()[0].body_id, body_id);
    assert_eq!(recovery.led_results()[0].wake_id, wake_id);
}

#[test]
fn bounded_multi_sign_transition_is_admitted_atomically() {
    let (mut recovery, _, _) = start();
    lose_websocket(&mut recovery);
    for sequence in 0..3 {
        recovery
            .refuse_replacement(
                HostId::from(conduit_net::R1_STD_HOST_ID),
                BootId::from(conduit_net::R1_STD_BOOT_ID),
                SignId::from(format!("r1/replan-requested-{sequence}")),
                SignId::from(format!("r1/no-realization-{sequence}")),
                PlanningRefusalReason::NoCompatibleRealization,
            )
            .unwrap();
    }
    assert_eq!(recovery.events().len(), MAX_R1_RECOVERY_EVENTS);
    let before = recovery.events().to_vec();
    assert_eq!(
        recovery.refuse_replacement(
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            SignId::from("r1/replan-requested-overflow"),
            SignId::from("r1/no-realization-overflow"),
            PlanningRefusalReason::NoCompatibleRealization,
        ),
        Err(R1RecoveryError::CapacityExhausted)
    );
    assert_eq!(recovery.events(), before);
}

#[test]
fn no_valid_replacement_remains_explicitly_unsatisfied() {
    let (mut recovery, _, _) = start();
    lose_websocket(&mut recovery);
    recovery
        .refuse_replacement(
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            SignId::from("r1/replan-requested"),
            SignId::from("r1/no-compatible-realization"),
            PlanningRefusalReason::NoCompatibleRealization,
        )
        .unwrap();
    assert_eq!(
        recovery.wake().lifecycle,
        conduit_body::WakeLifecycle::Unsatisfied
    );
    assert!(recovery.plan_b().is_none());
    assert!(matches!(
        recovery.events().last(),
        Some(ControlLoopEvent::PlanningRefused { .. })
    ));
}

#[test]
fn replacement_cannot_change_the_pico_realization_subject() {
    let (mut recovery, _, mut plan_b) = start();
    lose_websocket(&mut recovery);
    let pico = plan_b
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .find(|gear| gear.gear_id.as_str() == "signal-demo/show")
        .unwrap();
    pico.boot_id = BootId::from("r1/different-boot");
    assert!(matches!(
        recovery.install_replacement(
            plan_b,
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            HostId::from(conduit_net::R1_STD_HOST_ID),
            BootId::from(conduit_net::R1_STD_BOOT_ID),
            1,
            R1ReplacementSigns {
                request: SignId::from("r1/replan-requested"),
                planned: SignId::from("r1/plan-b-planned"),
                superseded: SignId::from("r1/plan-a-superseded"),
                realized: SignId::from("r1/plan-b-realized"),
                play_started: SignId::from("r1/play-b-started"),
            },
        ),
        Err(R1RecoveryError::InvalidPlan | R1RecoveryError::WrongRealizationSubject)
    ));
}
