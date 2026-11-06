#![cfg(feature = "kernel-operation-owners")]
//! Generic numerical conformance for the current authored full-float network.
//! Synthetic tensors are not model, utterance, streaming or listening evidence.
#[path = "fargan_numeric_conformance/full_frame.rs"]
mod full_frame;
#[path = "fargan_numeric_conformance/gate_order.rs"]
mod gate_order;
#[path = "fargan_numeric_conformance/matrices.rs"]
mod matrices;
#[allow(dead_code)]
#[path = "fargan_signal_graph/plan.rs"]
mod plan;
#[allow(dead_code)]
#[path = "fargan_signal_graph/runtime.rs"]
mod runtime;
#[allow(dead_code)]
#[path = "fargan_signal_graph/shared.rs"]
mod shared;
#[allow(dead_code)]
#[path = "fargan_signal_graph/state.rs"]
mod state;
use std::{cell::RefCell, collections::BTreeMap};
include!("../../../architecture/plot/tests/common/allocation_probe.rs");
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

fn on_large_stack(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap();
}
fn run_measured(
    plan: conduit_core::Plan,
    resources: &shared::Resources,
    values: BTreeMap<String, Vec<u8>>,
    mode: runtime::ExecutionMode,
) -> (Option<runtime::EncodedResultAndTiming>, Observation) {
    let interval = RefCell::new(None);
    let observation = RefCell::new(None);
    let result = runtime::run_entry_plan_observed(
        plan,
        resources,
        values,
        mode,
        || *interval.borrow_mut() = Some(ObservationInterval::begin()),
        || *observation.borrow_mut() = Some(interval.borrow_mut().take().unwrap().finish()),
    );
    let observation = observation.into_inner().expect("the Play phase ended");
    assert_eq!(
        (
            observation.allocation_attempts,
            observation.allocations,
            observation.reallocations
        ),
        (0, 0, 0)
    );
    (result, observation)
}
#[test]
fn callback_measurement_observes_real_growth_and_stops_before_cleanup() {
    let interval = ObservationInterval::begin();
    let storage = std::hint::black_box(Vec::<u8>::with_capacity(17));
    let observation = interval.finish();
    assert_eq!(observation.allocation_attempts, 1);
    assert_eq!(observation.allocations, 1);
    assert!(observation.requested_bytes >= 17);
    drop(storage);
    let (_, fresh) = observe(|| ());
    assert_eq!(fresh.allocation_attempts, 0);
}
