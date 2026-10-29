//! Actual finite Root renewal proof with explicit CPU hardware/clock adapters.
//! No MCU stack, electrical, flash durability or elapsed-16h claim.
#![allow(unexpected_cfgs)]
// Actual target modules intentionally include the same private wire/scope files.
#![cfg_attr(feature = "local-appliance", allow(clippy::duplicate_mod))]
// Actual target modules retain compatible entrances unused by this CPU route;
// generated immutable Source emits mutable builders for other feature profiles.
#![cfg_attr(
    feature = "local-appliance",
    allow(dead_code, unused_imports, unused_mut)
)]
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[cfg(feature = "local-appliance")]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_box_backing.rs"]
mod native_box_backing;
#[cfg(feature = "local-appliance")]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_offer_order.rs"]
mod native_offer_order;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

#[path = "native_root_renewal/manual_clock.rs"]
mod manual_clock;
use core::sync::atomic::{AtomicBool, Ordering};
static TIMER_COMPLETED: AtomicBool = AtomicBool::new(false);
#[embassy_executor::task]
async fn clock_probe() {
    embassy_time::Timer::after_millis(60_000).await;
    TIMER_COMPLETED.store(true, Ordering::SeqCst);
}
#[test]
fn bounded_cpu_clock_drives_actual_embassy_task_and_timer_without_wall_clock_sleep() {
    let executor = manual_clock::Executor::new();
    let start = embassy_time::Instant::now().as_millis();
    executor.spawner().spawn(clock_probe().unwrap());
    executor.poll_ready();
    assert!(!TIMER_COMPLETED.load(Ordering::SeqCst));
    assert!(executor.advance_next_timer());
    assert!(TIMER_COMPLETED.load(Ordering::SeqCst));
    assert_eq!(embassy_time::Instant::now().as_millis() - start, 60_000);
}

// `cargo rustc --test native_root_renewal -- --cfg ...` applies native profile
// flags to this test only, keeping dependency feature graphs unchanged.
#[cfg(feature = "local-appliance")]
include!("native_root_renewal/actual.rs");
