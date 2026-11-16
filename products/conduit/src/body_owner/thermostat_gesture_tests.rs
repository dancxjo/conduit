//! Normalized input fixtures through a real terminal Mask/kernel and running owner.
//! Terminal bytes and directional TLS Lines are deterministic in-memory effects,
//! not physical remote, live carrier, screen-reader or human enactment evidence.
use super::*;
use conduit_human::{
    ButtonSequenceOutcome, ButtonSequenceStep, InputButtonPhase, InputButtonTransition,
};
use conduit_std_host::terminal_face_mask::{
    TerminalButtonSequence, TerminalFaceMask, TerminalInputOutcome, TerminalKeyboard,
    TerminalMaskExecution,
};

const DOUBLE: [ButtonSequenceStep<'static>; 4] = [
    ButtonSequenceStep {
        button_identity: "remote/confirm",
        pressed: true,
    },
    ButtonSequenceStep {
        button_identity: "remote/confirm",
        pressed: false,
    },
    ButtonSequenceStep {
        button_identity: "remote/confirm",
        pressed: true,
    },
    ButtonSequenceStep {
        button_identity: "remote/confirm",
        pressed: false,
    },
];
fn button(pressed: bool, sequence: u64) -> InputButtonTransition {
    InputButtonTransition::new(
        "remote/confirm".into(),
        if pressed {
            InputButtonPhase::Pressed
        } else {
            InputButtonPhase::Released
        },
        sequence,
    )
    .unwrap()
}

#[test]
fn gesture_and_keyboard_terminal_inputs_reach_the_same_running_owner_action() {
    let (mut owner, root) = fixture(256);
    let (peer, _foreground, receipt, request) = terminal_fixture::admitted_peer(&mut owner, &root);
    let mut worker = owner.start_thermostat_run(&root, 30_000).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while owner.current_play_id().is_none() {
        assert!(Instant::now() < deadline);
        assert!(!worker.progress(&mut owner, &root).unwrap());
        thread::sleep(Duration::from_millis(5));
    }
    let body = owner.session.evidence().body_id.clone();
    let plan = owner.session.realization().unwrap().plan.plan_id.clone();
    let play = owner.current_play_id().unwrap().clone();
    let [face_line, show_line, interaction_line] = terminal_fixture::terminal_lines(
        owner.host.advertisement(),
        peer.advertisement(),
        &receipt,
    );
    let mut returned_actions = Vec::new();
    for remote_sequence in [false, true] {
        let expires = crate::durable_host::current_time_millis().unwrap() + 60_000;
        let route = owner
            .seal_remote_terminal_mask_route(
                &receipt,
                &face_line,
                &show_line,
                &interaction_line,
                expires,
            )
            .unwrap();
        let face = owner.face_snapshot(&request).unwrap();
        let mut execution = peer.prepare_remote_terminal_mask_execution(&route).unwrap();
        let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).unwrap();
        let mut bytes = Vec::new();
        mask.present(&mut execution, &mut bytes).unwrap();
        let mut keyboard = TerminalKeyboard::default();
        for _ in 0..128 {
            if mask
                .focused()
                .is_some_and(|focus| face.actions[focus.action].identity == "thermostat.mode.heat")
            {
                break;
            }
            let show = mask.show().unwrap().clone();
            let tab = keyboard.push(b'\t').unwrap().unwrap();
            mask.input(tab, &show, 0).unwrap();
            execution.close_without_input().unwrap();
            mask.present(&mut execution, &mut bytes).unwrap();
        }
        assert_eq!(
            face.actions[mask.focused().unwrap().action].identity,
            "thermostat.mode.heat"
        );
        assert!(!bytes.is_empty());
        let show = mask.show().unwrap().clone();
        owner.acknowledge_native_mask_show(&request, &show).unwrap();
        let prior_revision = owner.thermostat_live.as_ref().unwrap().revision;
        let interaction = if remote_sequence {
            let mut cancelled = TerminalButtonSequence::prepare(&mask, &show, &DOUBLE).unwrap();
            assert!(cancelled
                .observe(&mut mask, &button(true, 0), 1)
                .unwrap()
                .interaction
                .is_none());
            assert_eq!(cancelled.cancel(), ButtonSequenceOutcome::Cancelled);
            assert!(cancelled.observe(&mut mask, &button(false, 1), 1).is_err());
            assert_eq!(
                owner.thermostat_live.as_ref().unwrap().revision,
                prior_revision
            );
            let mut shortcut = TerminalButtonSequence::prepare(&mask, &show, &DOUBLE).unwrap();
            for (sequence, pressed) in [(2, true), (3, true), (4, false), (5, true)] {
                let outcome = shortcut
                    .observe(&mut mask, &button(pressed, sequence), 1)
                    .unwrap();
                assert!(outcome.interaction.is_none());
                assert_eq!(
                    owner.thermostat_live.as_ref().unwrap().revision,
                    prior_revision
                );
            }
            let completed = shortcut.observe(&mut mask, &button(false, 6), 1).unwrap();
            assert_eq!(completed.recognition, ButtonSequenceOutcome::Recognized);
            assert!(shortcut.observe(&mut mask, &button(false, 7), 2).is_err());
            completed.interaction.unwrap()
        } else {
            let enter = keyboard.push(b'\r').unwrap().unwrap();
            let TerminalInputOutcome::Interaction(interaction) =
                mask.input(enter, &show, 1).unwrap()
            else {
                panic!("ordinary keyboard activation")
            };
            interaction
        };
        returned_actions.push((interaction.action_id.clone(), interaction.target.clone()));
        let returned = execution.interact(interaction).unwrap();
        owner
            .validate_native_mask_interaction(&request, &show, &returned.interaction)
            .unwrap();
        worker
            .submit_interaction(&owner, &show, &returned.interaction)
            .unwrap();
        while worker.output_pending() {
            assert!(Instant::now() < deadline);
            assert!(!worker.progress(&mut owner, &root).unwrap());
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            owner.thermostat_live.as_ref().unwrap().state.mode,
            Mode::Heat
        );
        assert!(owner.thermostat_live.as_ref().unwrap().revision > prior_revision);
        assert_eq!(owner.session.evidence().body_id, body);
        assert_eq!(owner.session.realization().unwrap().plan.plan_id, plan);
        assert_eq!(owner.current_play_id(), Some(&play));
        assert!(worker
            .submit_interaction(&owner, &show, &returned.interaction)
            .is_err());
    }
    assert_eq!(returned_actions[0], returned_actions[1]);
    worker.request_lull().unwrap();
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    std::fs::remove_dir_all(root).unwrap();
}
