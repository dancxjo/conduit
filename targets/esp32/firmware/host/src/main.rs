//! Checked ESP32 Host composition. No diagnostic sampler or private display loop.
#![no_std]
#![no_main]
#[cfg(not(feature = "native-host"))]
compile_error!("select the checked native-host construction");
extern crate alloc;
use alloc::format;
use embassy_executor::Spawner;
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, rng::TrngSource,
    timer::timg::TimerGroup,
};
mod panic_capture;
use static_cell::StaticCell;
#[cfg(assigned_workset)]
mod authority;
mod bootstrap;
mod equipment;
mod renderer;
mod runtime;
#[cfg(assigned_workset)]
#[allow(dead_code)]
mod workset {
    pub mod generated {
        include!(concat!(env!("OUT_DIR"), "/workset.rs"));
    }
    pub use generated::*;
    pub mod native_images {
        pub use crate::native_images::*;
    }
    pub mod renderer {
        pub use crate::config::*;
    }
    pub use conduit_mcu_host_runtime::native_values::prepare as values;
}
mod play;
mod play_run;
mod play_service;
mod transport {
    pub type Link = conduit_mcu_host_runtime::transport::Link<
        conduit_esp32_electrical::uart::Uart0Carrier<'static>,
    >;
}

#[allow(dead_code)]
mod native_images {
    include!(concat!(env!("OUT_DIR"), "/native_images.rs"));
}
mod resident;
#[allow(dead_code)]
mod config {
    include!(concat!(env!("OUT_DIR"), "/host.rs"));
    pub use EXECUTION as EXECUTION_DELEGATION;
    pub use PREPARATION as PREPARATION_DELEGATION;
    pub use WORK_POOL as RESIDENT_WORK_POOL;
}
static BOOTSTRAP: StaticCell<bootstrap::Carrier> = StaticCell::new();
esp_bootloader_esp_idf::esp_app_desc!();
#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    // One checked internal heap after reserved ROM data/stacks. The linker
    // proves region geometry, sole section occupancy and primary stack reserve.
    esp_alloc::heap_allocator!(#[link_section = ".dram2_uninit"] size: config::HEAP_BYTES);
    let timers = TimerGroup::new(p.TIMG0);
    let software = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timers.timer0, software.software_interrupt0);
    // Retain the actual entropy source for every protocol nonce this Boot.
    let _entropy = TrngSource::new(p.RNG, p.ADC1);
    let nonce = conduit_esp32_electrical::platform::fresh_boot_nonce().expect("physical entropy");
    let uid = conduit_esp32_electrical::platform::physical_uid();
    let host = format!(
        "esp32/efuse/{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        uid[0], uid[1], uid[2], uid[3], uid[4], uid[5]
    );
    let seed = BOOTSTRAP
        .init(bootstrap::Carrier::new())
        .load(&host)
        .expect("actual Host / IMAGE / configuration bound bootstrap");
    let carrier = conduit_esp32_electrical::uart::Uart0Carrier::new(
        p.UART0,
        p.GPIO1,
        p.GPIO3,
        esp_hal::uart::Config::default(),
    )
    .expect("exclusive checked UART0");
    // Moving these pins proves exclusive physical custody. They are not used
    // until the shared whole-workset admission binds the native providers.
    let equipment = config::own_equipment!(p);
    let future = core::pin::pin!(runtime::run(seed, host, nonce, carrier, equipment, p.FLASH));
    conduit_mcu_host_runtime::poll_boundary::separate(future).await
}
