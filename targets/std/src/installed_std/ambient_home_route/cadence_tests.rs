//! Home and an independently advancing ordinary timer share one original IMAGE.
use super::*;
use std::sync::atomic::Ordering;

struct Ticks(Vec<u64>);
impl BodyForeOutputAdapter for Ticks {
    fn deliver(&mut self, delivery: &ExternalForeDelivery) -> Result<(), String> {
        assert!(self.0.len() < self.0.capacity());
        self.0
            .push(conduit_time::decode_tick(&delivery.bytes).unwrap());
        Ok(())
    }
}

#[test]
fn ambient_home_shares_original_image_with_finite_cadence() {
    let mut host = common::host(64 * 1024 * 1024);
    let (plan, wake) = common::planned_with_cadence(&host, 2, true);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let started = Cell::new(0);
    let highwater = Cell::new(0);
    let mut capture = Capture {
        frames: Vec::with_capacity(2),
        pressure: 1,
        attempts: 0,
    };
    let mut ticks = Ticks(Vec::with_capacity(2));
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &RunControl::default(),
                keyboard: None,
            },
            &[
                common::clock_input(1_794_805_200_000),
                common::clock_input(1_794_805_260_000),
            ],
            true,
            &mut ticks,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                highwater.set(memory.load(Ordering::Acquire));
                started.set(started.get() + 1);
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
    assert_eq!(started.get(), 1);
    assert_eq!(ticks.0, [0, 1]);
    assert_eq!(capture.frames.len(), 2);
    assert_eq!(
        capture.frames[0].fragment().subjects,
        capture.frames[1].fragment().subjects
    );
    assert!(memory.load(Ordering::Acquire) > baseline);
    drop(result);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);

    // The combined receipt must still refuse before Started when one byte
    // short. Recreate the exact same plan and input sequence, not a smaller run.
    let mut host = common::host((highwater.get() - 1) as u64);
    let (plan, wake) = common::planned_with_cadence(&host, 2, true);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let result = host.run_selected_image_body_plan_with_face_to_with_start(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &RunControl::default(),
            keyboard: None,
        },
        &[
            common::clock_input(1_794_805_200_000),
            common::clock_input(1_794_805_260_000),
        ],
        true,
        &mut NoOutput,
        &mut NoObservation,
        &mut Vec::new(),
        &mut ThreadTimer,
        |_, _| panic!("underfunded combined Home/timer must not Start"),
    );
    assert!(result.is_err());
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}
