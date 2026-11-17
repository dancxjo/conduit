use super::*;
use std::sync::atomic::Ordering;

#[test]
fn exact_retained_started_runs_actual_cue_and_keeps_same_image_report_charge() {
    for (behavior, expected) in [
        (
            FakePlaybackBehavior::Success,
            HostCallDisposition::Completed,
        ),
        (
            FakePlaybackBehavior::DeviceBusy,
            HostCallDisposition::Denied,
        ),
        (
            FakePlaybackBehavior::OpenFailure,
            HostCallDisposition::Failed,
        ),
    ] {
        crate::hosted_audio::startup_chime::reset_test_attempts();
        let (mut host, mut session) = fixture::fixture(behavior);
        let proposed = session.realization().unwrap().clone();
        let ad = host.advertisement().clone();
        let counter = host.kernel_resources.image_memory_observer();
        let baseline = counter.load(Ordering::Acquire);
        let mut probe = None;
        let result = host.run_selected_image_body_plan_with_committed_startup_to(
            BodyRunRequest {
                wake: &proposed.wake,
                plan: &proposed.plan,
                control: &crate::RunControl::default(),
                keyboard: None,
            },
            &[],
            &mut Vec::new(),
            &mut crate::ThreadTimer,
            None,
            |play, wake| {
                assert!(CommittedBodyStartup::from_retained(
                    session.evidence(),
                    &proposed.plan,
                    play
                )
                .is_err());
                session
                    .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                    .unwrap();
                let witness =
                    CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)?;
                probe = Some(crate::allocation_probe::begin());
                Ok(witness)
            },
        );
        let allocations = probe.map(|probe| probe.finish());
        let report = result.unwrap();
        assert_eq!(
            allocations,
            Some(0),
            "fake playback kernel must retain all owned storage before Started"
        );
        assert_eq!(report.report().terminal, TerminalDisposition::Completed);
        assert!(
            report.report().failure.is_none(),
            "{:?}",
            report.report().failure
        );
        assert_eq!(report.report().startup_chimes.len(), 1);
        let cue = &report.report().startup_chimes[0];
        assert_eq!(cue.disposition, Some(expected));
        assert!(cue.kernel_accepted);
        assert_eq!(
            cue.failure.is_some(),
            expected != HostCallDisposition::Completed
        );
        assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 1);
        assert!(counter.load(Ordering::Acquire) > baseline);
        drop(report);
        assert_eq!(counter.load(Ordering::Acquire), baseline);
    }
}

#[test]
fn failed_or_untyped_started_ack_cannot_fire_the_actual_source() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, session) = fixture::fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let sequence = host.next_kernel_play_sequence;
    let mut observed = false;
    let result = host.run_selected_image_body_plan_to_with_start(
        BodyRunRequest {
            wake: &proposed.wake,
            plan: &proposed.plan,
            control: &crate::RunControl::default(),
            keyboard: None,
        },
        &mut Vec::new(),
        &mut crate::ThreadTimer,
        |_, _| {
            observed = true;
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!observed);
    assert_eq!(host.next_kernel_play_sequence, sequence);
    let result = host.run_selected_image_body_plan_with_committed_startup_to(
        BodyRunRequest {
            wake: &proposed.wake,
            plan: &proposed.plan,
            control: &crate::RunControl::default(),
            keyboard: None,
        },
        &[],
        &mut Vec::new(),
        &mut crate::ThreadTimer,
        None,
        |_, _| Err("actual retained Started failed".into()),
    );
    assert!(result.err().unwrap().contains("retained Started failed"));
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 0);
    assert!(host.kernel_resources.is_idle());
}

#[test]
fn cancellation_during_first_pcm_block_retires_pending_actual_request_without_replay() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, mut session) = fixture::fixture(FakePlaybackBehavior::Success);
    let proposed = session.realization().unwrap().clone();
    let ad = host.advertisement().clone();
    let control = crate::RunControl::default();
    let mut probe = None;
    crate::hosted_audio::startup_chime::cancel_after_first_test_block();
    let result = host.run_selected_image_body_plan_with_committed_startup_to(
        BodyRunRequest {
            wake: &proposed.wake,
            plan: &proposed.plan,
            control: &control,
            keyboard: None,
        },
        &[],
        &mut Vec::new(),
        &mut crate::ThreadTimer,
        None,
        |play, wake| {
            session
                .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                .unwrap();
            let witness =
                CommittedBodyStartup::from_retained(session.evidence(), &proposed.plan, play)?;
            probe = Some(crate::allocation_probe::begin());
            Ok(witness)
        },
    );
    let allocations = probe.map(|probe| probe.finish());
    let report = result.unwrap();
    assert_eq!(allocations, Some(0));
    assert!(matches!(
        report.report().terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    let receipt = &report.report().startup_chimes[0];
    assert_eq!(receipt.disposition, Some(HostCallDisposition::Cancelled));
    assert!(!receipt.kernel_accepted);
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 1);
    assert!(host.kernel_resources.is_idle());
}

#[test]
fn a_new_retained_wake_has_one_new_cue_and_preserves_original_body_identity() {
    crate::hosted_audio::startup_chime::reset_test_attempts();
    let (mut host, mut session) = fixture::fixture(FakePlaybackBehavior::Success);
    let ad = host.advertisement().clone();
    let body = session.evidence().body_id.clone();
    for wake_index in 0..2 {
        let proposed = session.realization().unwrap().clone();
        let report = host
            .run_selected_image_body_plan_with_committed_startup_to(
                BodyRunRequest {
                    wake: &proposed.wake,
                    plan: &proposed.plan,
                    control: &crate::RunControl::default(),
                    keyboard: None,
                },
                &[],
                &mut Vec::new(),
                &mut crate::ThreadTimer,
                None,
                |play, wake| {
                    session
                        .started(&ad.host_id, &ad.boot_id, play.clone(), wake.clone())
                        .unwrap();
                    let witness = CommittedBodyStartup::from_retained(
                        session.evidence(),
                        &proposed.plan,
                        play,
                    )?;
                    assert!(witness.eligible(StartupScope::Wake));
                    assert_eq!(witness.eligible(StartupScope::Body), wake_index == 0);
                    Ok(witness)
                },
            )
            .unwrap();
        assert_eq!(report.report().play.body_id, body);
        assert_eq!(
            report.report().startup_chimes[0].disposition,
            Some(HostCallDisposition::Completed)
        );
        session
            .lull(&ad.host_id, &ad.boot_id, Some(&report.report().play))
            .unwrap();
        drop(report);
        if wake_index == 0 {
            session
                .propose(proposed.plan.plots, &ad.host_id, &ad.boot_id)
                .unwrap();
        }
    }
    assert_eq!(crate::hosted_audio::startup_chime::test_attempts(), 2);
}
