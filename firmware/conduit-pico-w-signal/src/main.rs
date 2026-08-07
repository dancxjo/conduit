//! Conduit Pico W Signal firmware.
//!
//! Runs the Signal demo form on real RP2040 hardware, blinks the onboard CYW43
//! LED, and emits machine-readable receipts over USB CDC.
//! The default image runs the generated Pico-local plan without heap
//! allocation. The explicit `usb-remote` image uses one finite startup arena
//! for owned session identities; active transport remains statically bounded.
#![no_std]
#![no_main]

#[cfg(all(feature = "pico-local", feature = "usb-remote"))]
compile_error!("select exactly one Pico firmware mode");
#[cfg(not(any(feature = "pico-local", feature = "usb-remote")))]
compile_error!("select exactly one Pico firmware mode");

mod kernel;
mod bootsel;
mod radio;
mod receipts;
#[cfg(feature = "usb-remote")]
mod remote_signal;
mod signal_image;
#[cfg(feature = "usb-remote")]
mod startup_arena;
mod usb;
mod usb_link;

use aligned::{A4, Aligned};
use embassy_executor::Spawner;
#[cfg(feature = "usb-remote")]
use embassy_futures::join::join;
#[cfg(feature = "pico-local")]
use embassy_futures::select::{select, Either};
use panic_halt as _;

#[cfg(feature = "usb-remote")]
#[global_allocator]
static ALLOCATOR: startup_arena::StartupArena = startup_arena::StartupArena::new();

#[cfg(feature = "pico-local")]
struct NoAllocator;

#[cfg(feature = "pico-local")]
unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[cfg(feature = "pico-local")]
#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

// Vendored CYW43 firmware assets — checked at build time via xtask doctor.
static CYW43_FW: Aligned<A4, [u8; 231077]> = Aligned(*include_bytes!(
    "../../../firmware/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
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

    // Both modes expose the same dual-CDC physical shape. The local proof owns
    // CDC 1 only; the remote proof additionally installs CDC 0 as its carrier.
    let usb_driver = embassy_rp::usb::Driver::new(p.USB, radio::UsbIrq);
    let (usb_fut, link_carrier, evidence_sender) = usb::init_composite_usb(usb_driver);
    spawner.spawn(receipts::usb_task_spawn(usb_fut).unwrap());
    let runtime = receipts::RuntimeTranscriptIdentity::new();
    let mut cdc = receipts::UsbCdc::new(evidence_sender.sender);

    #[cfg(feature = "pico-local")]
    {
        let mut link_session = usb_link::UsbLinkSession::new(link_carrier).unwrap();
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
        // While the local proof is idle waiting for its evidence consumer,
        // CDC 0 remains an autonomous recovery path into BOOTSEL.
        match select(cdc.wait_dtr(), bootsel::wait_for_request(&mut link_session)).await {
            Either::First(()) => {}
            Either::Second(Ok(())) => unreachable!(),
            Either::Second(Err(_)) => core::future::pending::<()>().await,
        }
        kernel::run_signal_demo(&mut control, &mut cdc, &runtime).await;
        let _ = bootsel::wait_for_request(&mut link_session).await;
    }

    #[cfg(feature = "usb-remote")]
    {
    let mut link_session = usb_link::UsbLinkSession::new(link_carrier).unwrap();

    // Service the physical USB startup while CYW43 initializes. Enumeration is
    // not a live CDC service: both futures must be polled from the beginning.
    let usb_startup =
        remote_signal::establish_usb_channels(&mut link_session, &mut cdc, &runtime);
    let radio_startup = async {
        let (control, _) = radio::init_cyw43(
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
        control
    };
    let (usb_result, mut control) = join(usb_startup, radio_startup).await;
    if usb_result.is_err() {
        core::future::pending::<()>().await;
    }
    if cdc.write_marker("CONDUIT_CYW43_GPIO_READY").await.is_err() {
        core::future::pending::<()>().await;
    }

    // Execute the USB-CDC remote session sink
    let _ = remote_signal::run_remote_signal_sink(
        &mut link_session,
        &mut cdc,
        &mut control,
        &runtime,
    )
    .await;
    let _ = bootsel::wait_for_request(&mut link_session).await;
    }
}
