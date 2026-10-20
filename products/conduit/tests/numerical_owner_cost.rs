//! Complete actual numerical caller observation; run alone in a settled process.
#![allow(dead_code)]
#[path = "../src/plot_source.rs"]
mod plot_source;
#[path = "../src/numerical_run/preparation.rs"]
mod preparation;
use conduit_core::*;
use conduit_std_host::numerical_fore::NumericalFore;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicU64, Ordering::Relaxed},
    time::Instant,
};
struct Probe;
static REQUESTS: AtomicU64 = AtomicU64::new(0);
static REQUESTED: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
fn added(bytes: usize) {
    REQUESTS.fetch_add(1, Relaxed);
    REQUESTED.fetch_add(bytes as u64, Relaxed);
    let live = LIVE.fetch_add(bytes as u64, Relaxed) + bytes as u64;
    PEAK.fetch_max(live, Relaxed);
}
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            added(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            added(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as u64, Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size() as u64, Relaxed);
            added(size);
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
#[derive(Clone, Copy)]
struct Snapshot {
    requests: u64,
    requested: u64,
    live: u64,
}
fn snapshot() -> Snapshot {
    Snapshot {
        requests: REQUESTS.load(Relaxed),
        requested: REQUESTED.load(Relaxed),
        live: LIVE.load(Relaxed),
    }
}
fn delta(before: Snapshot, after: Snapshot) -> serde_json::Value {
    serde_json::json!({"allocation_requests":after.requests-before.requests,
        "requested_bytes":after.requested-before.requested,
        "live_bytes_delta":after.live as i64-before.live as i64})
}

fn owner() -> NumericalFore {
    let source = plot_source::parse(include_str!(
        "../../../plots/quantity-projection/main.conduit"
    ))
    .unwrap();
    let checked = source.check().unwrap();
    let authored = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "quantity-projection/main",
        source.authoring_catalog(),
    )
    .unwrap();
    let (host, plan) = preparation::prepare(&source, &checked, &authored).unwrap();
    NumericalFore::from_plan(host, plan).unwrap()
}
#[test]
#[ignore = "matched full-owner allocation/timing observation; run alone"]
fn observe_numerical_owner() {
    let input = Quantity::parse_plot_literal("0.3m").unwrap();
    let mut primer = owner();
    primer.execute(input).unwrap();
    drop(primer.finish().unwrap());
    let before = snapshot();
    PEAK.store(before.live, Relaxed);
    let cold_start = Instant::now();
    let mut execution = owner();
    let cold_ns = cold_start.elapsed().as_nanos();
    let prepared = snapshot();
    let mut warm_ns = [0_u128; 32];
    let mut output_bytes = 0;
    for time in &mut warm_ns {
        let started = Instant::now();
        let output = execution.execute(input).unwrap();
        *time = started.elapsed().as_nanos();
        output_bytes += output.canonical.len();
    }
    let executed = snapshot();
    let finish_start = Instant::now();
    let report = execution.finish().unwrap();
    let finish_ns = finish_start.elapsed().as_nanos();
    let finished = snapshot();
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    let receipts = report.each_child_signs.as_ref().unwrap();
    assert_eq!(receipts.len(), 32);
    let events: usize = receipts.iter().map(|r| r.events.len()).sum();
    let sidecars: usize = receipts.iter().map(|r| r.remote_lifecycle.len()).sum();
    assert!(events > 0 && sidecars > 0);
    drop(report);
    let disposed = snapshot();
    let peak = PEAK.load(Relaxed) - before.live;
    println!(
        "NUMERICAL_OWNER_COST={}",
        serde_json::json!({
            "schema":"conduit.numerical-fore-owner-observation@1", "values":32,
            "cold":delta(before,prepared), "warm_values":delta(prepared,executed),
            "finish_and_inspection":delta(executed,finished),
            "inspection_and_disposal":delta(finished,disposed),
            "complete_owner":delta(before,disposed), "peak_live_bytes_over_baseline":peak,
            "cold_ns":cold_ns, "warm_ns":warm_ns, "finish_ns":finish_ns,
            "input_payload_bytes":input.encode().len()*32, "output_payload_bytes":output_bytes,
            "retained_child_events":events, "retained_remote_sidecars":sidecars,
        })
    );
}
