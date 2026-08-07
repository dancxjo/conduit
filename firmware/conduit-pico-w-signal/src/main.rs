//! Conduit Pico W Signal firmware.
//!
//! Runs the Signal demo form on real RP2040 hardware, blinks the onboard CYW43
//! LED, and emits machine-readable receipts over USB CDC.
//! Startup-owned identities use one finite static arena; active transport and
//! execution storage remain statically bounded.
#![no_std]
#![no_main]

mod kernel;
mod radio;
mod receipts;
mod remote_signal;
mod signal_image;
mod usb;
mod usb_link;

use aligned::{A4, Aligned};
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use panic_halt as _;
use portable_atomic::{AtomicUsize, Ordering};

const STARTUP_ARENA_BYTES: usize = 16 * 1024;

struct StartupArena {
    bytes: UnsafeCell<[u8; STARTUP_ARENA_BYTES]>,
    next: AtomicUsize,
}

unsafe impl Sync for StartupArena {}

impl StartupArena {
    const fn new() -> Self {
        Self {
            bytes: UnsafeCell::new([0; STARTUP_ARENA_BYTES]),
            next: AtomicUsize::new(0),
        }
    }
}

unsafe impl GlobalAlloc for StartupArena {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = self.bytes.get().cast::<u8>() as usize;
        let mut current = self.next.load(Ordering::Relaxed);
        loop {
            let Some(aligned_address) = (base + current)
                .checked_add(layout.align() - 1)
                .map(|address| address & !(layout.align() - 1))
            else {
                return core::ptr::null_mut();
            };
            let aligned_offset = aligned_address - base;
            let Some(next) = aligned_offset.checked_add(layout.size()) else {
                return core::ptr::null_mut();
            };
            if next > STARTUP_ARENA_BYTES {
                return core::ptr::null_mut();
            }
            match self
                .next
                .compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => return aligned_address as *mut u8,
                Err(observed) => current = observed,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOCATOR: StartupArena = StartupArena::new();

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

    // Initialise USB CDC receipt & link channels
    let usb_driver = embassy_rp::usb::Driver::new(p.USB, radio::UsbIrq);
    let (usb_fut, link_carrier, evidence_sender) = usb::init_composite_usb(usb_driver);
    spawner.spawn(receipts::usb_task_spawn(usb_fut).unwrap());
    let runtime = receipts::RuntimeTranscriptIdentity::new();
    let mut cdc = receipts::UsbCdc::new(evidence_sender.sender);
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
    cdc.write_log("CONDUIT_CYW43_GPIO_READY").await;

    // Execute the USB-CDC remote session sink
    let _ = remote_signal::run_remote_signal_sink(
        &mut link_session,
        &mut cdc,
        &mut control,
        &runtime,
    )
    .await;
}
