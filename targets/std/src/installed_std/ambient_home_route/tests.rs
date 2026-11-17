//! Real authored Home Plan refusal before Start; no display/IMAGE success claim.
use crate::{
    body_execution::{
        BodyFaceContributionObserver, BodyForeOutputAdapter, BodyRunRequest,
        CommittedFaceContribution, ReturnedFaceContribution,
    },
    ExternalForeDelivery, ExternalForeInput, RunControl, ThreadTimer,
};
use conduit_ambient_display::PreparedAmbientFaces;
use conduit_core::{ConnectionTrack, PortDirection, StructuredInfoValue};
use conduit_face::{FaceContributionBasis, FaceFragment};
use std::{cell::Cell, collections::BTreeMap};
#[allow(dead_code)]
#[path = "../../../../../plots/ambient-display/model/src/tests/fixtures.rs"]
mod fixtures;

struct NoObservation;
impl BodyFaceContributionObserver for NoObservation {
    fn contribute(&mut self, _: CommittedFaceContribution) -> Result<(), ReturnedFaceContribution> {
        panic!("unfunded Home must not publish");
    }
}
struct NoOutput;
impl BodyForeOutputAdapter for NoOutput {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        panic!("Home has no executable output");
    }
}
#[test]
fn ambient_home_plan_requires_original_image_execution_entrance() {
    let mut host = common::host(64 * 1024 * 1024);
    let (plan, wake) = common::planned(&host, 2);
    let input = common::input();
    let started = Cell::new(false);
    let result = host.run_body_plan_with_fore_and_face_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[input],
        false,
        &mut NoOutput,
        &mut NoObservation,
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| {
            started.set(true);
            Ok(())
        },
    );
    let error = result.unwrap_err();
    assert_eq!(
        error,
        "selected IMAGE workset lacks complete ordinary managed-memory accounting; no Play admitted"
    );
    assert!(!started.get());
}

mod common;

struct Capture {
    frames: Vec<CommittedFaceContribution>,
    pressure: usize,
    attempts: usize,
}
impl BodyFaceContributionObserver for Capture {
    fn contribute(
        &mut self,
        value: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        self.attempts += 1;
        if self.pressure > 0 {
            self.pressure -= 1;
            return Err(ReturnedFaceContribution {
                contribution: value,
                refusal: crate::body_execution::FaceContributionRefusal::Pressure,
            });
        }
        assert!(self.frames.len() < self.frames.capacity());
        self.frames.push(value);
        Ok(())
    }
}
#[test]
fn ambient_home_original_image_retains_pressure_and_escaped_frame_charge() {
    use std::sync::atomic::Ordering;
    let mut host = common::host(64 * 1024 * 1024);
    let (plan, wake) = common::planned(&host, 2);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let started = Cell::new(0);
    let highwater = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(2),
        pressure: 3,
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
            &[common::input(), common::input()],
            true,
            &mut NoOutput,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                started.set(started.get() + 1);
                highwater.set(memory.load(Ordering::Acquire));
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
    assert_eq!(started.get(), 1, "refresh must not restart the Play");
    assert_eq!(capture.frames.len(), 2);
    assert_eq!(capture.attempts, 5);
    assert_eq!(capture.frames[0].fragment().subjects[0].name, "Home");
    assert_eq!(
        capture.frames[0].fragment().subjects,
        capture.frames[1].fragment().subjects
    );
    let retained = memory.load(Ordering::Acquire);
    assert!(baseline < retained && retained <= highwater.get());
    drop(result);
    assert_eq!(memory.load(Ordering::Acquire), retained);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}

#[test]
fn ambient_home_one_byte_short_original_image_refuses_before_start() {
    use std::sync::atomic::Ordering;
    let highwater = Cell::new(0usize);
    {
        let mut host = common::host(64 * 1024 * 1024);
        let (plan, wake) = common::planned(&host, 2);
        let memory = host.kernel_resources.image_memory_observer();
        let mut capture = Capture {
            frames: Vec::with_capacity(1),
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
                &[common::input()],
                false,
                &mut NoOutput,
                &mut capture,
                &mut Vec::new(),
                &mut ThreadTimer,
                |_, _| {
                    highwater.set(memory.load(Ordering::Acquire));
                    Ok(())
                },
            )
            .unwrap();
        assert!(
            result.report().failure.is_none(),
            "{:?}",
            result.report().failure
        );
        assert_eq!(capture.frames.len(), 1);
    }
    assert!(highwater.get() > crate::ambient_home_offer::workspace_bytes().unwrap() as usize + 1);
    let mut host = common::host((highwater.get() - 1) as u64);
    let (plan, wake) = common::planned(&host, 2);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let started = Cell::new(false);
    let mut capture = Capture {
        frames: Vec::with_capacity(1),
        pressure: 0,
        attempts: 0,
    };
    let result = host.run_selected_image_body_plan_with_face_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[common::input()],
        false,
        &mut NoOutput,
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
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}

mod live_tests;

mod cadence_tests;

mod clock_source_tests;
mod retention_tests;

mod aspect_tests;
mod resident_tests;

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
#[path = "native_worker_tests.rs"]
mod native_worker_tests;
