use super::*;

#[test]
fn four_checked_ticks_run_without_post_started_allocation() {
    let (mut host, wake, plan, mut clock) = fixture(4, 64 * 1024 * 1024);
    let control = crate::RunControl::default();
    let mut output = Output::new(4);
    let mut text = Vec::with_capacity(4096);
    let mut probe = None;
    let result = host.run_selected_image_body_plan_with_output_to_with_start(
        request(&wake, &plan, &control),
        &mut text,
        &mut clock,
        Some(&mut output),
        |_, _| {
            probe = Some(crate::allocation_probe::begin());
            Ok(())
        },
    );
    assert_eq!(probe.unwrap().finish(), 0);
    let report = result.unwrap();
    assert!(matches!(
        report.report().terminal,
        TerminalDisposition::Completed
    ));
    assert_eq!(output.retained.len(), 4);
    for (sequence, value) in output.retained.iter().enumerate() {
        assert_eq!(
            conduit_time::decode_tick(&value.bytes).unwrap(),
            sequence as u64
        );
    }
}

#[test]
fn cancellation_and_provider_loss_use_admitted_diagnostics_and_output() {
    for provider_loss in [false, true] {
        let (mut host, wake, plan, mut clock) = fixture(4, 64 * 1024 * 1024);
        let control = crate::RunControl::default();
        let lost = clock.lost.clone();
        let mut output = Output::new(4);
        let mut text = Vec::with_capacity(4096);
        let mut probe = None;
        let mut stop = Some(crate::RunControlRequestId::new("funded-test-stop").unwrap());
        let result = host.run_selected_image_body_plan_with_output_to_with_start(
            request(&wake, &plan, &control),
            &mut text,
            &mut clock,
            Some(&mut output),
            |_, _| {
                if provider_loss {
                    lost.set(true);
                } else {
                    control.request_stop(stop.take().unwrap()).unwrap();
                }
                probe = Some(crate::allocation_probe::begin());
                Ok(())
            },
        );
        assert_eq!(probe.unwrap().finish(), 0);
        let report = result.unwrap();
        if provider_loss {
            assert!(matches!(
                report.report().terminal,
                TerminalDisposition::Failed { .. }
            ));
            assert!(report.report().failure.is_some());
        } else {
            assert!(matches!(
                report.report().terminal,
                TerminalDisposition::Cancelled { .. }
            ));
        }
        assert!(host.kernel_resources.is_idle());
    }
}

#[test]
fn actual_thread_timer_uses_the_same_funded_public_body_path() {
    let (mut host, wake, plan, _) = fixture(3, 64 * 1024 * 1024);
    let control = crate::RunControl::default();
    let mut clock = crate::ThreadTimer;
    let mut output = Output::new(3);
    let mut text = Vec::with_capacity(4096);
    let mut probe = None;
    let result = host.run_selected_image_body_plan_with_output_to_with_start(
        request(&wake, &plan, &control),
        &mut text,
        &mut clock,
        Some(&mut output),
        |_, _| {
            probe = Some(crate::allocation_probe::begin());
            Ok(())
        },
    );
    assert_eq!(probe.unwrap().finish(), 0);
    let report = result.unwrap();
    assert!(matches!(
        report.report().terminal,
        TerminalDisposition::Completed
    ));
    assert_eq!(output.retained.len(), 3);
    assert!(!report.report().clock_observations.is_empty());
}

#[test]
fn zero_count_keeps_the_existing_finite_tick_contract() {
    let (mut host, wake, plan, mut clock) = fixture(0, 64 * 1024 * 1024);
    let control = crate::RunControl::default();
    let mut output = Output::new(1);
    let mut text = Vec::new();
    let report = host
        .run_selected_image_body_plan_with_output_to_with_start(
            request(&wake, &plan, &control),
            &mut text,
            &mut clock,
            Some(&mut output),
            |_, _| Ok(()),
        )
        .unwrap();
    assert!(matches!(
        report.report().terminal,
        TerminalDisposition::Completed
    ));
    assert!(output.retained.is_empty());
}
