//! Ordinary authored timer -> actual clock Host Call -> same Home Face.
use super::*;

#[test]
fn ambient_home_clock_source_exhaustion_does_not_restart_or_publish_an_extra_update() {
    let mut host = common::clock_host(64 * 1024 * 1024);
    let (plan, wake) = common::planned_clock_limits(&host, 2, 3);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let starts = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(2),
        pressure: 0,
        attempts: 0,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[],
            false,
            &mut NoOutput,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(starts.get(), 1);
    assert_eq!(capture.frames.len(), 2);
    assert!(result.report().failure.is_some());
    assert_eq!(
        result.report().kernel_failure,
        Some(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::WorkBudgetExhausted,
            detail: 81,
        })
    );
    assert!(matches!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Failed { .. }
    ));
    drop(result);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}
use std::{
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn ambient_home_authored_clock_source_samples_after_start_without_fore_injection() {
    let mut host = common::clock_host(64 * 1024 * 1024);
    let (plan, wake) = common::planned_clock(&host, 2);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let started = Cell::new(0);
    let start_ms = Cell::new(0);
    let highwater = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(2),
        pressure: 1,
        attempts: 0,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[],
            false,
            &mut NoOutput,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                started.set(started.get() + 1);
                start_ms.set(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                );
                highwater.set(memory.load(Ordering::Acquire));
                Ok(())
            },
        )
        .unwrap();
    let end_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert_eq!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert_eq!(started.get(), 1);
    assert_eq!(capture.frames.len(), 2);
    assert_eq!(
        capture.frames[0].fragment().subjects,
        capture.frames[1].fragment().subjects
    );
    for frame in &capture.frames {
        let observed = frame
            .fragment()
            .properties
            .iter()
            .find(|property| property.name == "observed-at-unix-ms")
            .unwrap();
        let conduit_face::FacePropertyValue::Count(observed) = observed.value else {
            panic!("clock timestamp type")
        };
        assert!((start_ms.get()..=end_ms).contains(&observed));
    }
    drop(result);
    assert!(memory.load(Ordering::Acquire) > baseline);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);

    let mut host = common::clock_host((highwater.get() - 1) as u64);
    let (plan, wake) = common::planned_clock(&host, 2);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let result = host.run_selected_image_body_plan_with_face_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[],
        false,
        &mut NoOutput,
        &mut NoObservation,
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| panic!("underfunded clock-source workset must not Start"),
    );
    assert!(result.is_err());
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}
