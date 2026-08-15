//! Finite generated Signal image for the inspected HW-463 / ESP-WROOM-32.
//!
//! This build checkpoint proves PROFILE -> BUILD -> IMAGE only. Physical
//! BOOT/HOST receipts are added and accepted separately.

#![no_std]
#![no_main]
#![deny(clippy::large_stack_frames, clippy::mem_forget)]

use esp_hal::{
    clock::CpuClock,
    main,
    time::{Duration, Instant},
};

mod generated {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/signal_image.rs"));
}

const _: () = assert!(generated::GENERATED_NODES.len() == 2);
const _: () = assert!(generated::GENERATED_CORDS.len() == 1);
const _: () = assert!(generated::GENERATED_ROUTES.len() == 1);
const _: () = assert!(generated::GENERATED_ROUTE_TARGETS.len() == 1);
const _: () = assert!(generated::GENERATED_HOST_OPERATIONS.len() == 2);
const _: () = assert!(generated::GENERATED_RESOURCES.len() == 2);
const _: () = assert!(generated::CORD_VALUE_SLOTS == 1);
const _: () = assert!(generated::CORD_VALUE_BYTES == 9);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let _peripherals = esp_hal::init(config);

    // Keep the generated image resident. This build-only checkpoint does not
    // emit a BOOT or HOST receipt and therefore cannot be mistaken for one.
    let _exact_kernel_node_specs: &[conduit_kernel::scheduler::NodeSpec<
        { generated::GENERATED_PORTS_PER_NODE },
    >] = &generated::GENERATED_NODES;
    loop {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(500) {
            core::hint::spin_loop();
        }
    }
}
