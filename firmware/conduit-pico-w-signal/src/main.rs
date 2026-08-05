//! Conduit Pico W Signal firmware.
//!
//! Runs the Signal demo form on real RP2040 hardware, blinks the onboard CYW43
//! LED, and emits machine-readable receipts over USB CDC.
//! No runtime heap allocator is used; all storage is statically sized.
#![no_std]
#![no_main]

mod kernel;
mod radio;
mod receipts;
mod signal_image;

use aligned::{A4, Aligned};
use embassy_executor::Spawner;
use panic_halt as _;

// Vendored CYW43 firmware assets — checked at build time via xtask doctor.
static CYW43_FW: Aligned<A4, [u8; 231077]> = Aligned(*include_bytes!(
    "../../../firmware/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
const CYW43_CLM: &[u8] = include_bytes!(
    "../../../firmware/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_clm.bin"
);
static CYW43_NVRAM: Aligned<A4, [u8; 742]> = Aligned(*include_bytes!(
    "../../../firmware/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"
));
// License is an identity input to the firmware build.
const _CYW43_LICENSE: &[u8] = include_bytes!(
    "../../../firmware/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/LICENSE-permissive-binary-license-1.0.txt"
);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Initialise USB CDC receipt channel
    let usb_driver = embassy_rp::usb::Driver::new(p.USB, radio::UsbIrq);
    let (usb_fut, mut cdc) = receipts::init_usb(usb_driver);
    spawner.spawn(receipts::usb_task_spawn(usb_fut).unwrap());

    // Initialise CYW43 radio (required for onboard LED)
    let (mut control, _) = radio::init_cyw43(
        &spawner,
        p.PIO0,
        p.DMA_CH0,
        p.PIN_23,
        p.PIN_24,
        p.PIN_25,
        p.PIN_29,
        &CYW43_FW,
        &CYW43_NVRAM,
    )
    .await;

    // Feed CLM blob to radio
    control.init(CYW43_CLM).await;
    control.set_power_management(cyw43::PowerManagementMode::PowerSave).await;

    // Execute the Signal demo through the Conduit kernel
    kernel::run_signal_demo(&mut control, &mut cdc).await;
}
