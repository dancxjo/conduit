//! Local matched observation of the ordinary Source -> Plan -> Body owner.
use conduit_thermostat_plot::{Command, ThermostatState};
use conduit_thermostat_runtime::execution::Execution;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

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
#[test]
#[ignore = "local owner cost observation; run alone, not a timing gate"]
fn observe_ordinary_body_owner() {
    let before = snapshot();
    PEAK.store(before.live, Relaxed);
    let mut execution = Execution::new().unwrap();
    let prepared = snapshot();
    let mut expected = ThermostatState::default();
    for index in 0..32 {
        let command = Command::SetTarget(180 + (index % 8) * 5);
        expected = expected.apply(command).unwrap();
        assert_eq!(execution.execute(command).unwrap().state, expected);
    }
    let executed = snapshot();
    let report = execution.finish().unwrap();
    let finished = snapshot();
    assert_eq!(
        report.terminal,
        conduit_core::TerminalDisposition::Completed
    );
    let receipts = report.scan_child_signs.as_ref().unwrap();
    assert_eq!(receipts.len(), 32);
    let events: usize = receipts.iter().map(|receipt| receipt.events.len()).sum();
    let sidecars: usize = receipts
        .iter()
        .map(|receipt| receipt.remote_lifecycle.len())
        .sum();
    assert!(events > 0 && sidecars > 0);
    drop(report);
    let disposed = snapshot();
    let peak = PEAK.load(Relaxed) - before.live;
    // Formatting occurs after every measured phase and final owner disposal.
    println!(
        "BODY_OWNER_COST={}",
        serde_json::json!({
            "schema":"conduit.body-value-owner-observation@1", "values":32,
            "cold":delta(before,prepared), "warm_values":delta(prepared,executed),
            "finish_and_inspection":delta(executed,finished),
            "inspection_and_disposal":delta(finished,disposed),
            "complete_owner":delta(before,disposed), "peak_live_bytes_over_baseline":peak,
            "retained_child_events":events, "retained_remote_sidecars":sidecars
        })
    );
}
