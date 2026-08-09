use conduit_core::{
    BootId, ClueId, ControlLoopEvent, GearId, HostId, LineAvailability, LineAvailabilitySign,
    PlanningRefusalReason,
};
use conduit_system_continuity::{
    exact_r1_signal_plan, R1LedResultObservation, R1NewPlanRecovery, R1RecoveryError,
    R1RecoveryStartClues, R1ReplacementClues, R1SignalRouteSet, MAX_R1_RECOVERY_EVENTS,
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
        GearId::from("show"),
        1,
        1,
        HostId::from(conduit_net::R1_STD_HOST_ID),
        BootId::from(conduit_net::R1_STD_BOOT_ID),
        0,
        R1RecoveryStartClues {
            birth: ClueId::from("r1/body-born"),
            wake: ClueId::from("r1/body-woke"),
            plan_ready: ClueId::from("r1/plan-a-ready"),
            play_started: ClueId::from("r1/play-a-started"),
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
                sign_id: ClueId::from("r1/websocket-unavailable"),
            },
            ClueId::from("r1/play-a-unsatisfied"),
        )
        .unwrap();
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
            R1ReplacementClues {
                request: ClueId::from("r1/replan-requested"),
                planned: ClueId::from("r1/plan-b-planned"),
                superseded: ClueId::from("r1/plan-a-superseded"),
                realized: ClueId::from("r1/plan-b-realized"),
                play_started: ClueId::from("r1/play-b-started"),
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
        Some(conduit_body::WakeLifecycleEvent::Replanned { clue_id, .. })
            if clue_id.as_str() == "r1/plan-b-planned"
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
            clue_id: ClueId::from("r1/stale-led-result"),
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
            clue_id: ClueId::from("r1/plan-b-led-on"),
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
            clue_id: ClueId::from("r1/stale-observed-session"),
            level: true,
        }),
        Err(R1RecoveryError::StaleResult)
    );
    assert_eq!(recovery.led_results()[0].body_id, body_id);
    assert_eq!(recovery.led_results()[0].wake_id, wake_id);
}

#[test]
fn bounded_multi_clue_transition_is_admitted_atomically() {
    let (mut recovery, _, _) = start();
    lose_websocket(&mut recovery);
    for sequence in 0..3 {
        recovery
            .refuse_replacement(
                HostId::from(conduit_net::R1_STD_HOST_ID),
                BootId::from(conduit_net::R1_STD_BOOT_ID),
                ClueId::from(format!("r1/replan-requested-{sequence}")),
                ClueId::from(format!("r1/no-realization-{sequence}")),
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
            ClueId::from("r1/replan-requested-overflow"),
            ClueId::from("r1/no-realization-overflow"),
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
            ClueId::from("r1/replan-requested"),
            ClueId::from("r1/no-compatible-realization"),
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
        .find(|gear| gear.gear_id.as_str() == "show")
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
            R1ReplacementClues {
                request: ClueId::from("r1/replan-requested"),
                planned: ClueId::from("r1/plan-b-planned"),
                superseded: ClueId::from("r1/plan-a-superseded"),
                realized: ClueId::from("r1/plan-b-realized"),
                play_started: ClueId::from("r1/play-b-started"),
            },
        ),
        Err(R1RecoveryError::InvalidPlan | R1RecoveryError::WrongRealizationSubject)
    ));
}
