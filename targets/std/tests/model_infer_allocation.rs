//! Diagnostic measurement, not a storage-admission or zero-allocation claim.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_core::ConnectionTrack;
use conduit_data::NamedTensorBundle;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::hosted_model_infer::prepare_linear_model;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, KernelTimingPhase,
    TimerAdapter,
};
use fixture::*;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::Duration,
};
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static REQUESTED: Cell<usize> = const { Cell::new(0) };
}
fn record(bytes: usize) {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = CALLS.try_with(|v| v.set(v.get().saturating_add(1)));
            let _ = REQUESTED.try_with(|v| v.set(v.get().saturating_add(bytes)));
        }
    });
}
struct Counted;
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(size);
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
#[derive(Default)]
struct Probe {
    phases: u8,
    calls: usize,
    requested: usize,
}
impl TimerAdapter for Probe {
    fn wait(&mut self, _: Duration) {
        panic!("no timer wait");
    }
    fn observe_kernel_phase(&mut self, phase: KernelTimingPhase) {
        match phase {
            KernelTimingPhase::PreparationStarted => {}
            KernelTimingPhase::PlayStarted => {
                assert_eq!(self.phases, 0);
                self.phases = 1;
                CALLS.with(|v| v.set(0));
                REQUESTED.with(|v| v.set(0));
                ENABLED.with(|v| v.set(true));
            }
            KernelTimingPhase::PlayFinished => {
                ENABLED.with(|v| v.set(false));
                self.calls = CALLS.with(Cell::get);
                self.requested = REQUESTED.with(Cell::get);
                self.phases = 2;
            }
        }
    }
}
struct Sink {
    bytes: Box<[u8; 262144]>,
    length: usize,
    calls: usize,
}
impl ExternalForeOutputAdapter for Sink {
    fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
        if delivery.track == ConnectionTrack::Payload {
            if delivery.bytes.len() > self.bytes.len() {
                return Err("output exceeds admitted maximum".into());
            }
            self.bytes[..delivery.bytes.len()].copy_from_slice(&delivery.bytes);
            self.length = delivery.bytes.len();
            self.calls += 1;
        }
        Ok(())
    }
}
#[test]
fn characterize_actual_model_play_allocation_requests() {
    let (mut host, plan) = host_plan(include_str!("fixtures/model-linear.conduit"), |signature| {
        Box::new(
            prepare_linear_model(model(signature), offer(), runtime("linear"), requirement())
                .unwrap(),
        )
    });
    let inputs = [ExternalForeInput {
        front_port_id: "inputs".into(),
        track: ConnectionTrack::Payload,
        bytes: bundle(input([2., 3.])).encode().unwrap(),
    }];
    let mut sink = Sink {
        bytes: Box::new([0; 262144]),
        length: 0,
        calls: 0,
    };
    let mut probe = Probe::default();
    let result = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &inputs,
        &mut sink,
        &mut Vec::new(),
        &mut probe,
    );
    ENABLED.with(|v| v.set(false));
    result.unwrap();
    assert_eq!(probe.phases, 2);
    assert_eq!(sink.calls, 1);
    let output = NamedTensorBundle::decode(&sink.bytes[..sink.length]).unwrap();
    assert_eq!(output.get()[0].name, "output");
    println!("model_play_allocation_requests={{\"calls\":{},\"requested_bytes_sum\":{},\"output_bytes\":{}}}", probe.calls, probe.requested, sink.length);
}
