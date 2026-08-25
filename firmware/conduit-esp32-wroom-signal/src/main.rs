//! Finite generated Signal image and optional bounded BLE Base for the
//! inspected HW-463 / ESP-WROOM-32.

#![no_std]
#![no_main]
#![deny(clippy::large_stack_frames, clippy::mem_forget)]

#[cfg(not(feature = "kernel-signal"))]
compile_error!("select the checked kernel-signal Base feature");

extern crate alloc;

use embassy_executor::Spawner;
use esp_alloc as _;
use esp_backtrace as _;
#[cfg(feature = "bluetooth")]
use esp_hal::rng::{Trng, TrngSource};
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup,
};
#[cfg(feature = "bluetooth")]
use esp_radio::ble::controller::BleConnector;
#[cfg(feature = "bluetooth")]
use trouble_host::prelude::ExternalController;

#[cfg(feature = "bluetooth")]
mod bluetooth;
#[cfg(feature = "bluetooth")]
mod receipts;
#[cfg(feature = "bluetooth")]
mod session;

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
const _: () = assert!(!generated::GENERATED_FABRICATION_DESCRIPTOR_BINDING.is_empty());

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let timer_group = TimerGroup::new(peripherals.TIMG0);
    let software_interrupts = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timer_group.timer0, software_interrupts.software_interrupt0);
    // Keep the descriptor-bound generated image resident. Its planner input
    // used explicitly synthetic fixture identities. This build-only checkpoint
    // emits no BOOT or HOST receipt and therefore cannot be mistaken for one.
    let _exact_kernel_node_specs: &[conduit_kernel::scheduler::NodeSpec<
        { generated::GENERATED_PORTS_PER_NODE },
    >] = &generated::GENERATED_NODES;
    #[cfg(feature = "bluetooth")]
    {
        let _trng_source = TrngSource::new(peripherals.RNG, peripherals.ADC1);
        let mut trng = Trng::try_new().expect("the physical entropy source must initialize");
        let boot = receipts::BootIdentity::fresh(&trng);
        boot.print_boot();
        let connector = BleConnector::new(peripherals.BT, Default::default())
            .expect("the inspected ESP32 BLE controller must initialize");
        let controller: ExternalController<_, 1> = ExternalController::new(connector);
        bluetooth::run(controller, &boot, &mut trng).await;
    }

    #[cfg(not(feature = "bluetooth"))]
    core::future::pending::<()>().await;
}
