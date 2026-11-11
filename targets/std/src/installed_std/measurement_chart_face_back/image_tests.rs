//! Real checked CPU IMAGE admission and escaped immutable Face custody.
use super::tests::{input, planned, planned_for_update_bound, Capture, Outputs};
use crate::{body_execution::BodyRunRequest, RunControl, ThreadTimer};
use std::{cell::Cell, sync::atomic::Ordering};

#[test]
fn selected_image_face_tokens_keep_exact_original_charge_after_report_drops() {
    let mut host = crate::image_initialization::cpu_image_test_fixture(64 * 1024 * 1024);
    let (plan, wake) = planned(host.advertisement());
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    let mut capture = Capture {
        frames: Vec::with_capacity(1),
        pressure: 1,
        final_pressure: 0,
        attempts: 0,
    };
    let highwater = Cell::new(0);
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[input(100)],
            false,
            &mut Outputs,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                highwater.set(observed.load(Ordering::Acquire));
                Ok(())
            },
        )
        .unwrap();
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert_eq!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert_eq!(capture.frames.len(), 1);
    let retained = observed.load(Ordering::Acquire);
    assert!(baseline < retained && retained <= highwater.get());
    drop(result);
    assert_eq!(
        observed.load(Ordering::Acquire),
        retained,
        "escaped token must retain SAME original result charge"
    );
    drop(capture);
    assert_eq!(observed.load(Ordering::Acquire), baseline);
}

#[test]
fn one_byte_short_selected_face_receipt_refuses_before_start_without_losing_charge() {
    let highwater = Cell::new(0);
    {
        let mut host = crate::image_initialization::cpu_image_test_fixture(64 * 1024 * 1024);
        let (plan, wake) = planned_for_update_bound(host.advertisement(), 128);
        let observed = host.kernel_resources.image_memory_observer();
        let mut capture = Capture {
            frames: Vec::with_capacity(1),
            pressure: 0,
            final_pressure: 0,
            attempts: 0,
        };
        host.run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[input(100)],
            false,
            &mut Outputs,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                highwater.set(observed.load(Ordering::Acquire));
                Ok(())
            },
        )
        .unwrap();
    }
    let cpu_pool = conduit_std_offers::measurement_plot_workspace_bytes() as usize
        + conduit_std_offers::measurement_chart_face_workspace_bytes() as usize;
    assert!(
        highwater.get() > cpu_pool + 1,
        "fixture must reach execution admission, not fail CPU initialization"
    );
    let mut host =
        crate::image_initialization::cpu_image_test_fixture((highwater.get() - 1) as u64);
    let (plan, wake) = planned_for_update_bound(host.advertisement(), 128);
    let observed = host.kernel_resources.image_memory_observer();
    let baseline = observed.load(Ordering::Acquire);
    let mut capture = Capture {
        frames: Vec::with_capacity(1),
        pressure: 0,
        final_pressure: 0,
        attempts: 0,
    };
    let started = Cell::new(false);
    let result = host.run_selected_image_body_plan_with_face_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[input(100)],
        false,
        &mut Outputs,
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| {
            started.set(true);
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!started.get());
    assert!(capture.frames.is_empty());
    assert_eq!(observed.load(Ordering::Acquire), baseline);
}
