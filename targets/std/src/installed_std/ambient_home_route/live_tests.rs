//! Actual after-Started ingress, not a production timer/network producer.
use super::*;
use crate::body_execution::{ImageForeInput, ImageLiveForeQueue};
use crate::{BodyLiveForeAdmission, BodyLiveForeQueue, RunControlRequestId};
use std::{
    mem::size_of,
    sync::{atomic::Ordering, Arc},
};

// Independently audit the synchronous constructor's allocating operations with
// original SAFE primitives. This fixed one-fragment fixture has no workers or
// effects during construction. Shared Plan/control/producer/probe bookkeeping
// remain test-owned. Sum of ALL requested bytes bounds its construction peak,
// including realloc overlap and destruction; this is not a generic allowance.
fn preparation_envelope(
    host: &mut crate::StdHost,
    wake: &conduit_body::Wake,
    plan: &conduit_body::BodyPlan,
    control: &RunControl,
) -> usize {
    use crate::{body_live_fore::image::ImageLiveAdmission, kernel_preparation::ImageMemoryLease};
    let probe = crate::allocation_probe::begin();
    let memory = host
        .kernel_resources
        .reserve_managed_base_memory(ImageMemoryLease::owned_backing_bytes())
        .unwrap();
    let play = host.prospective_body_play(wake, plan).unwrap();
    let queue = BodyLiveForeQueue::for_local_closing_flow_fragment(
        &plan.plots[0].plan.fragments[0],
        "observations",
        control.clone(),
        1,
        2,
    )
    .unwrap();
    queue
        .bind_image_admission(ImageLiveAdmission {
            host: host.advertisement().host_id.clone(),
            boot: host.advertisement().boot_id.clone(),
            offer: host.advertisement().offer_generation,
            play,
            memory,
        })
        .unwrap();
    let queue = Arc::new(queue);
    let _retained = size_of::<BodyLiveForeQueue>()
        + 2 * size_of::<usize>()
        + queue.image_clone_metadata_bytes().unwrap()
        + queue.image_shared_backing_bytes().unwrap()
        + ImageMemoryLease::owned_backing_bytes();
    // The production shrink step is three bounded atomic accounting operations,
    // with no allocation. Do not execute/relabel this diagnostic raw queue.
    drop(queue);
    probe
        .finish_statistics()
        .requested_bytes
        .checked_add(ImageMemoryLease::owned_backing_bytes())
        .unwrap()
}
struct LiveCapture<'a> {
    queue: ImageLiveForeQueue,
    second: &'a ImageForeInput,
    frames: Vec<CommittedFaceContribution>,
}
impl BodyFaceContributionObserver for LiveCapture<'_> {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        assert!(!frame.scope().is_retired());
        if self.frames.is_empty() {
            assert_eq!(
                self.queue.submit(&self.second.input().bytes).unwrap(),
                BodyLiveForeAdmission::Accepted { sequence: 1 }
            );
            self.queue.close().unwrap();
        }
        assert!(self.frames.len() < self.frames.capacity());
        self.frames.push(frame);
        Ok(())
    }
}
#[test]
fn ambient_home_live_input_changes_clock_without_restart_and_keeps_original_custody() {
    let mut host = common::host(64 * 1024 * 1024);
    let (plan, wake) = common::planned(&host, 2);
    let control = RunControl::default();
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    // Codec/preparation temporaries belong to the test producer; returned inputs
    // then keep their actual retained payload on the ORIGINAL IMAGE counter.
    let inputs = [
        common::clock_input(1_794_805_200_000),
        common::clock_input(1_794_805_260_000),
    ]
    .into_iter()
    .map(|input| host.retain_selected_image_fore_input(input).unwrap())
    .collect::<Vec<_>>();
    let before = memory.load(Ordering::Acquire);
    let envelope = preparation_envelope(&mut host, &wake, &plan, &control);
    assert_eq!(memory.load(Ordering::Acquire), before);
    let probe = crate::allocation_probe::begin();
    // SAFETY: the audited complete synchronous constructor above qualifies this
    // EXACT immutable Plan/control/1slot/2input fixture, with no worker/effect.
    // The original entry reserves that envelope BEFORE allocating. Its actual
    // requested peak is checked below; no cross-fixture envelope is reused.
    let queue = unsafe {
        host.prepare_selected_image_live_fore_queue(
            envelope,
            &wake,
            &plan,
            control.clone(),
            "observations",
            1,
            2,
        )
    }
    .unwrap();
    assert!(probe.finish_statistics().peak_live_bytes <= envelope);
    assert!(queue.submit(&inputs[0].input().bytes).is_err());
    let charged = memory.load(Ordering::Acquire);
    let mut capture = LiveCapture {
        queue: queue.clone(),
        second: &inputs[1],
        frames: Vec::with_capacity(2),
    };
    assert_eq!(
        memory.load(Ordering::Acquire),
        charged,
        "queue clone must not reserve again"
    );
    let starts = Cell::new(0);
    let result = std::thread::scope(|threads| {
        let producer = queue.clone();
        let first = &inputs[0];
        let stop = control.clone();
        // The producer worker is separately test-owned, with a finite stack.
        let worker = std::thread::Builder::new()
            .name("ambient-live-proof".into())
            .stack_size(512 * 1024)
            .spawn_scoped(threads, move || {
                let result = (|| -> Result<(), String> {
                    producer.wait_until_play_started()?;
                    if producer.submit(&first.input().bytes)?
                        != (BodyLiveForeAdmission::Accepted { sequence: 0 })
                    {
                        return Err("first live Home input not accepted".into());
                    }
                    Ok(())
                })();
                if result.is_err() {
                    let _ = stop.request_stop(
                        RunControlRequestId::new("home/live-producer-failed").unwrap(),
                    );
                }
                result
            })
            .unwrap();
        let result = host.run_selected_image_body_plan_with_live_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &queue,
            &mut NoOutput,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                Ok(())
            },
        );
        if result.is_err() {
            let _ = control.request_stop(RunControlRequestId::new("home/live-run-failed").unwrap());
        }
        worker.join().unwrap().unwrap();
        result
    })
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
    assert_eq!(capture.frames.len(), 2);
    assert_eq!(
        capture.frames[0].fragment().subjects,
        capture.frames[1].fragment().subjects
    );
    let clock = |f: &CommittedFaceContribution| {
        f.fragment()
            .properties
            .iter()
            .find(|p| p.name == "local-time")
            .unwrap()
            .value
            .clone()
    };
    assert_ne!(clock(&capture.frames[0]), clock(&capture.frames[1]));
    assert_eq!(queue.status().kernel_admitted, 2);
    assert!(queue.status().kernel_closed && queue.status().play_terminal);
    assert!(queue.submit(&inputs[1].input().bytes).is_err());
    let retained = memory.load(Ordering::Acquire);
    drop(result);
    assert_eq!(memory.load(Ordering::Acquire), retained);
    drop(capture);
    drop(queue);
    drop(inputs);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}
