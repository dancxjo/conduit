//! Two ordinary presentation Plots share the original IMAGE and observer.
use super::*;
use conduit_ambient_display::AmbientDisplayAspect;
use std::sync::atomic::Ordering;

fn assert_aspects(frames: &[CommittedFaceContribution]) {
    assert_eq!(frames.len(), 4);
    for label in ["Wallpaper", "Clock and weather"] {
        let matching = frames
            .iter()
            .filter(|f| f.fragment().subjects[0].name == label)
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 2);
        assert_eq!(
            matching[0].fragment().subjects,
            matching[1].fragment().subjects
        );
        assert!(matching
            .iter()
            .all(|f| !f.fragment().subjects.iter().any(|s| s.name == "Home")));
        let forbidden = if label == "Wallpaper" {
            "clock-weather"
        } else {
            "wallpaper"
        };
        assert!(matching.iter().all(|f| {
            !f.fragment()
                .subjects
                .iter()
                .any(|s| s.identity.ends_with(forbidden))
        }));
    }
    assert!(frames
        .iter()
        .all(|f| f.fragment().basis == frames[0].fragment().basis));
    assert_ne!(
        frames
            .iter()
            .find(|f| f.fragment().subjects[0].name == "Wallpaper")
            .unwrap()
            .fragment()
            .subjects[0]
            .identity,
        frames
            .iter()
            .find(|f| f.fragment().subjects[0].name == "Clock and weather")
            .unwrap()
            .fragment()
            .subjects[0]
            .identity
    );
}

#[test]
fn two_aspect_plot_backs_share_atomic_input_custody_pressure_and_one_image_start() {
    let mut host = common::aspects_host(64 * 1024 * 1024, false);
    let (plan, wake) = common::planned_aspects(&host, 2, false);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let starts = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(4),
        pressure: 2,
        attempts: 0,
    };
    let inputs = [
        common::clock_input(1_794_805_200_000),
        common::clock_input(1_794_805_260_000),
    ];
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &inputs,
            true,
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
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert_eq!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Completed
    );
    assert_eq!(starts.get(), 1);
    assert!(capture.attempts >= 6);
    assert_aspects(&capture.frames);
    drop(result);
    assert!(memory.load(Ordering::Acquire) > baseline);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}

#[test]
fn one_actual_observer_fans_out_to_both_plots_without_another_start() {
    let mut host = common::aspects_host(64 * 1024 * 1024, true);
    let (plan, wake) = common::planned_aspects(&host, 2, true);
    let placements = plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments)
        .flat_map(|f| &f.placements)
        .collect::<Vec<_>>();
    assert_eq!(
        placements
            .iter()
            .filter(|p| p.kind_id.as_str() == conduit_ambient_display::AMBIENT_OBSERVE_KIND)
            .count(),
        1
    );
    for aspect in [
        AmbientDisplayAspect::Wallpaper,
        AmbientDisplayAspect::ClockWeather,
    ] {
        assert_eq!(
            placements
                .iter()
                .filter(|p| p.kind_id.as_str() == aspect.kind())
                .count(),
            1
        );
    }
    let starts = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(4),
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
                starts.set(starts.get() + 1);
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
    assert_eq!(starts.get(), 1);
    assert_aspects(&capture.frames);
}

#[test]
fn cancelling_two_aspects_at_start_publishes_neither_and_recovers_original_memory() {
    let mut host = common::aspects_host(64 * 1024 * 1024, false);
    let (plan, wake) = common::planned_aspects(&host, 2, false);
    let control = RunControl::default();
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let starts = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(4),
        pressure: 0,
        attempts: 0,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &[common::clock_input(1_794_805_200_000)],
            true,
            &mut NoOutput,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                control
                    .request_stop(crate::RunControlRequestId::new("aspects/cancel").unwrap())
                    .unwrap();
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(starts.get(), 1);
    assert!(capture.frames.is_empty());
    assert!(matches!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    drop(result);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}
