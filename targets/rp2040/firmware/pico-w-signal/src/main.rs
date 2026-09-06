//! Conduit Pico W Signal firmware.
//!
//! Runs the Signal demo form on real RP2040 hardware, blinks the onboard CYW43
//! LED, and emits machine-readable receipts over USB CDC.
//! The default image runs the generated Pico-local plan without heap
//! allocation. The explicit `usb-remote` image uses one finite startup arena
//! for owned session identities; active transport remains statically bounded.
//! `pico-local-minimal` is a compile-only composition proof with the same
//! kernel-backed Signal faces and USB sign, but no wire/session or BOOTSEL
//! lifecycle-control base. It is not a substitute for physical acceptance.
#![no_std]
#![no_main]

#[cfg(any(
    all(feature = "pico-local", feature = "usb-remote"),
    all(feature = "pico-local", feature = "triple-remote"),
    all(feature = "pico-local", feature = "pico-local-minimal"),
    all(feature = "pico-local-minimal", feature = "usb-remote"),
    all(feature = "pico-local-minimal", feature = "triple-remote"),
    all(feature = "usb-remote", feature = "triple-remote"),
    all(feature = "wifi-bootstrap", feature = "pico-local"),
    all(feature = "wifi-bootstrap", feature = "pico-local-minimal"),
    all(feature = "wifi-bootstrap", feature = "usb-remote"),
    all(feature = "wifi-bootstrap", feature = "triple-remote"),
    all(feature = "appliance-hello", feature = "pico-local"),
    all(feature = "appliance-hello", feature = "pico-local-minimal"),
    all(feature = "appliance-hello", feature = "usb-remote"),
    all(feature = "appliance-hello", feature = "triple-remote"),
    all(feature = "appliance-hello", feature = "wifi-bootstrap"),
    all(feature = "appliance-hil-client", feature = "pico-local"),
    all(feature = "appliance-hil-client", feature = "pico-local-minimal"),
    all(feature = "appliance-hil-client", feature = "usb-remote"),
    all(feature = "appliance-hil-client", feature = "triple-remote"),
    all(feature = "appliance-hil-client", feature = "wifi-bootstrap"),
    all(feature = "appliance-hil-client", feature = "appliance-hello"),
    all(feature = "bluetooth-line", feature = "pico-local"),
    all(feature = "bluetooth-line", feature = "pico-local-minimal"),
    all(feature = "bluetooth-line", feature = "usb-remote"),
    all(feature = "bluetooth-line", feature = "triple-remote"),
    all(feature = "bluetooth-line", feature = "wifi-bootstrap"),
    all(feature = "bluetooth-line", feature = "appliance-hello"),
    all(feature = "bluetooth-line", feature = "appliance-hil-client"),
    all(feature = "distributed-lenia", feature = "pico-local"),
    all(feature = "distributed-lenia", feature = "pico-local-minimal"),
    all(feature = "distributed-lenia", feature = "usb-remote"),
    all(feature = "distributed-lenia", feature = "triple-remote"),
    all(feature = "distributed-lenia", feature = "wifi-bootstrap"),
    all(feature = "distributed-lenia", feature = "appliance-hello"),
    all(feature = "distributed-lenia", feature = "appliance-hil-client"),
    all(feature = "distributed-lenia", feature = "bluetooth-line")
    , all(feature = "light-switch", feature = "pico-local")
    , all(feature = "light-switch", feature = "pico-local-minimal")
    , all(feature = "light-switch", feature = "usb-remote")
    , all(feature = "light-switch", feature = "triple-remote")
    , all(feature = "light-switch", feature = "wifi-bootstrap")
    , all(feature = "light-switch", feature = "appliance-hello")
    , all(feature = "light-switch", feature = "appliance-hil-client")
    , all(feature = "light-switch", feature = "bluetooth-line")
    , all(feature = "light-switch", feature = "distributed-lenia")
))]
compile_error!("select exactly one Pico firmware mode");
#[cfg(not(any(
    feature = "pico-local",
    feature = "pico-local-minimal",
    feature = "usb-remote",
    feature = "triple-remote",
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "bluetooth-line",
    feature = "distributed-lenia"
    , feature = "light-switch"
)))]
compile_error!("select exactly one Pico firmware mode");

#[cfg(feature = "appliance-hello")]
mod appliance;
#[cfg(feature = "appliance-hil-client")]
mod appliance_hil_client;
#[cfg(feature = "bluetooth-line")]
mod bluetooth_line;
#[cfg(any(feature = "pico-local", feature = "wifi-bootstrap"))]
mod body_admission;
mod embedded_spore;
#[cfg(all(feature = "session-control", not(feature = "light-switch")))]
mod bootsel;
#[cfg(feature = "wifi-bootstrap")]
mod continuable_signal;
#[cfg(feature = "distributed-lenia")]
mod distributed_lenia;
#[cfg(not(any(
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "bluetooth-line",
    feature = "distributed-lenia",
    feature = "light-switch"
)))]
mod kernel;
#[cfg(feature = "distributed-lenia")]
mod lenia_image;
#[cfg(all(feature = "light-switch", not(feature = "indicator-resource")))]
mod light_switch;
#[cfg(feature = "indicator-resource")]
mod indicator_resource;
#[cfg(feature = "wifi-bootstrap")]
mod network_image;
#[cfg(feature = "wifi-bootstrap")]
mod network_operations;
#[cfg(feature = "wifi-bootstrap")]
mod network_receipts;
#[cfg(feature = "wifi-bootstrap")]
mod panic_recovery;
#[cfg(feature = "wifi-bootstrap")]
mod plan_b_signal_image;
#[cfg(feature = "wifi-bootstrap")]
mod plan_c_signal_image;
#[cfg_attr(feature = "light-switch", allow(unused_imports))]
mod radio;
#[cfg_attr(feature = "light-switch", allow(dead_code))]
mod receipts;
#[cfg(all(
    any(feature = "session-control", feature = "bluetooth-line"),
    not(feature = "light-switch")
))]
mod remote_error;
#[cfg(any(
    feature = "usb-remote",
    feature = "triple-remote",
    feature = "wifi-bootstrap",
    feature = "bluetooth-line"
))]
mod remote_kernel;
#[cfg(any(
    feature = "usb-remote",
    feature = "triple-remote",
    feature = "wifi-bootstrap"
))]
mod remote_signal;
#[cfg(any(
    feature = "usb-remote",
    feature = "triple-remote",
    feature = "wifi-bootstrap",
    feature = "bluetooth-line"
))]
mod signal_execution_identity;
#[cfg(not(any(
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "distributed-lenia",
    feature = "light-switch"
)))]
mod signal_image;
#[cfg(feature = "wifi-bootstrap")]
mod signal_recovery;
#[cfg(any(
    feature = "usb-remote",
    feature = "triple-remote",
    feature = "wifi-bootstrap",
    feature = "bluetooth-line"
))]
mod startup_arena;
mod usb;
#[cfg(all(feature = "session-control", not(feature = "light-switch")))]
mod usb_link;
#[cfg(feature = "wifi-bootstrap")]
mod websocket_route;
#[cfg(feature = "wifi-bootstrap")]
mod websocket_signal;
#[cfg(feature = "wifi-bootstrap")]
mod websocket_transport;
#[cfg(feature = "wifi-bootstrap")]
mod wifi_join;
#[cfg(feature = "wifi-bootstrap")]
mod wifi_recovery;
#[cfg(feature = "wifi-bootstrap")]
mod wifi_session;

use aligned::{Aligned, A4};
use embassy_executor::Spawner;
#[cfg(any(feature = "usb-remote", feature = "triple-remote"))]
use embassy_futures::join::join;
#[cfg(any(
    feature = "pico-local",
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "bluetooth-line",
    feature = "distributed-lenia"
))]
use embassy_futures::select::{select, Either};
#[cfg(any(feature = "bluetooth-line", feature = "distributed-lenia"))]
use embassy_rp::flash::Flash;
#[cfg(not(feature = "wifi-bootstrap"))]
use panic_halt as _;

#[cfg(any(
    feature = "usb-remote",
    feature = "triple-remote",
    feature = "wifi-bootstrap",
    feature = "bluetooth-line"
))]
#[global_allocator]
static ALLOCATOR: startup_arena::StartupArena = startup_arena::StartupArena::new();

#[cfg(any(
    feature = "pico-local",
    feature = "pico-local-minimal",
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "distributed-lenia",
    feature = "light-switch"
))]
struct NoAllocator;

#[cfg(any(
    feature = "pico-local",
    feature = "pico-local-minimal",
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "distributed-lenia",
    feature = "light-switch"
))]
unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[cfg(any(
    feature = "pico-local",
    feature = "pico-local-minimal",
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "distributed-lenia",
    feature = "light-switch"
))]
#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

// Vendored CYW43 firmware assets — checked at build time via xtask doctor.
static CYW43_FW: Aligned<A4, [u8; 231077]> = Aligned(*include_bytes!(
    "../../../firmware-assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
static CYW43_NVRAM: Aligned<A4, [u8; 742]> = Aligned(*include_bytes!(
    "../../../firmware-assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"
));
#[cfg(any(feature = "bluetooth-line", feature = "distributed-lenia"))]
static CYW43_BTFW: Aligned<A4, [u8; 6164]> = Aligned(*include_bytes!(
    "../../../firmware-assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_btfw.bin"
));
#[cfg(any(
    feature = "wifi-bootstrap",
    feature = "appliance-hello",
    feature = "appliance-hil-client",
    feature = "bluetooth-line",
    feature = "distributed-lenia"
))]
static CYW43_CLM: &[u8; 984] = include_bytes!(
    "../../../firmware-assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0_clm.bin"
);
// License is an identity input to the firmware build.
const _CYW43_LICENSE: &[u8] = include_bytes!(
    "../../../firmware-assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/LICENSE-permissive-binary-license-1.0.txt"
);

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    #[cfg(feature = "wifi-bootstrap")]
    let panic_record = panic_recovery::take(p.WATCHDOG);

    // Physical-proof and remote modes expose dual CDC: CDC 0 owns session and
    // lifecycle control, while CDC 1 owns sign. The minimal composition
    // omits that optional family and exposes one sign-only CDC interface.
    let usb_driver = embassy_rp::usb::Driver::new(p.USB, radio::UsbIrq);
    #[cfg(feature = "session-control")]
    let (usb_fut, session_line, sign_sender) = usb::init_composite_usb(usb_driver);
    #[cfg(not(feature = "session-control"))]
    let (usb_fut, sign_sender) = usb::init_sign_usb(usb_driver);
    spawner.spawn(receipts::usb_task_spawn(usb_fut).unwrap());
    #[cfg(all(
        not(feature = "wifi-bootstrap"),
        not(feature = "appliance-hello"),
        not(feature = "appliance-hil-client"),
        not(feature = "distributed-lenia")
        , not(feature = "light-switch")
    ))]
    let runtime =
        receipts::RuntimeTranscriptIdentity::new(signal_image::PLAN_ID, signal_image::HOST_ID);
    #[cfg(feature = "distributed-lenia")]
    let runtime =
        receipts::RuntimeTranscriptIdentity::new(lenia_image::PLAN_ID, lenia_image::HOST_ID);
    #[cfg(feature = "wifi-bootstrap")]
    let runtime =
        receipts::RuntimeTranscriptIdentity::new(network_image::PLAN_ID, network_image::HOST_ID);
    let mut cdc = receipts::UsbCdc::new(sign_sender.sender);

    #[cfg(feature = "light-switch")]
    {
        let (mut control, _) = radio::init_cyw43(
            &spawner,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            &CYW43_FW,
            &CYW43_NVRAM,
        )
        .await;
        #[cfg(not(feature = "indicator-resource"))]
        light_switch::run(session_line, &mut cdc, &mut control).await;
        #[cfg(feature = "indicator-resource")]
        indicator_resource::run(session_line, &mut control).await;
    }

    #[cfg(feature = "pico-local")]
    {
        let mut link_session = usb_link::UsbLinkSession::new(session_line).unwrap();
        let (mut control, _) = radio::init_cyw43(
            &spawner,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            &CYW43_FW,
            &CYW43_NVRAM,
        )
        .await;
        // While the local proof is idle waiting for its sign consumer,
        // CDC 0 remains an autonomous recovery path into BOOTSEL.
        let mut admission = body_admission::PicoBodyAdmission::new(&runtime);
        loop {
            match select(cdc.wait_dtr(), admission.serve_once(&mut link_session)).await {
                Either::First(()) => break,
                Either::Second(Ok(())) => {}
                Either::Second(Err(_)) => core::future::pending::<()>().await,
            }
        }
        kernel::run_signal_demo(&mut control, &mut cdc, &runtime).await;
        let _ = bootsel::wait_for_request(&mut link_session).await;
    }

    #[cfg(feature = "pico-local-minimal")]
    {
        let (mut control, _) = radio::init_cyw43(
            &spawner,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            &CYW43_FW,
            &CYW43_NVRAM,
        )
        .await;
        cdc.wait_dtr().await;
        kernel::run_signal_demo(&mut control, &mut cdc, &runtime).await;
        core::future::pending::<()>().await;
    }

    #[cfg(any(feature = "usb-remote", feature = "triple-remote"))]
    {
        let mut link_session = usb_link::UsbLinkSession::new(session_line).unwrap();

        // Service the physical USB startup while CYW43 initializes. Enumeration is
        // not a live CDC service: both futures must be polled from the beginning.
        let usb_startup =
            remote_signal::establish_usb_channels(&mut link_session, &mut cdc, &runtime);
        let radio_startup = async {
            let (control, _) = radio::init_cyw43(
                &spawner,
                p.PIO0,
                p.DMA_CH0,
                p.DMA_CH1,
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

        // Execute the USB-CDC remote session sink.
        let result = remote_signal::run_remote_signal_sink(
            &mut link_session,
            &mut cdc,
            &mut control,
            &runtime,
        )
        .await;
        if let Err(error) = result {
            let _ = cdc
                .write_failure(error.code(), kernel::terminal_identity(), &runtime)
                .await;
        }
        let _ = bootsel::wait_for_request(&mut link_session).await;
    }

    #[cfg(feature = "wifi-bootstrap")]
    {
        wifi_join::run(
            &spawner,
            session_line,
            &mut cdc,
            panic_record,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            &CYW43_FW,
            &CYW43_NVRAM,
            CYW43_CLM,
            &runtime,
        )
        .await;
    }

    #[cfg(feature = "appliance-hello")]
    {
        let mut link_session = usb_link::UsbLinkSession::new(session_line).unwrap();
        let services = appliance::run(
            &spawner,
            &mut cdc,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            &CYW43_FW,
            &CYW43_NVRAM,
            CYW43_CLM,
        );
        let recovery = async {
            loop {
                let _ = bootsel::wait_for_request(&mut link_session).await;
            }
        };
        match select(services, recovery).await {
            Either::First(value) => value,
            Either::Second(value) => value,
        }
    }

    #[cfg(feature = "appliance-hil-client")]
    {
        let mut link_session = usb_link::UsbLinkSession::new(session_line).unwrap();
        let probe = appliance_hil_client::run(
            &spawner,
            &mut cdc,
            p.PIO0,
            p.DMA_CH0,
            p.DMA_CH1,
            p.PIN_23,
            p.PIN_24,
            p.PIN_25,
            p.PIN_29,
            &CYW43_FW,
            &CYW43_NVRAM,
            CYW43_CLM,
        );
        let recovery = async {
            loop {
                let _ = bootsel::wait_for_request(&mut link_session).await;
            }
        };
        match select(probe, recovery).await {
            Either::First(value) => value,
            Either::Second(value) => value,
        }
    }

    #[cfg(feature = "bluetooth-line")]
    {
        const PICO_W_FLASH_BYTES: usize = 2 * 1024 * 1024;
        let mut link_session = usb_link::UsbLinkSession::new(session_line).unwrap();
        let mut flash = Flash::<_, _, PICO_W_FLASH_BYTES>::new_blocking(p.FLASH);
        let mut flash_unique_id = [0_u8; 8];
        flash
            .blocking_unique_id(&mut flash_unique_id)
            .expect("Pico W flash must expose its physical unique identity");
        let bluetooth = async {
            cdc.wait_dtr().await;
            bluetooth_line::run(
                &spawner,
                &mut cdc,
                p.PIO0,
                p.DMA_CH0,
                p.DMA_CH1,
                p.PIN_23,
                p.PIN_24,
                p.PIN_25,
                p.PIN_29,
                &CYW43_FW,
                &CYW43_BTFW,
                &CYW43_NVRAM,
                CYW43_CLM,
                &runtime,
                flash_unique_id,
            )
            .await
        };
        let recovery = async {
            loop {
                let _ = bootsel::wait_for_request(&mut link_session).await;
            }
        };
        match select(bluetooth, recovery).await {
            Either::First(value) => value,
            Either::Second(value) => value,
        }
    }

    #[cfg(feature = "distributed-lenia")]
    {
        const PICO_W_FLASH_BYTES: usize = 2 * 1024 * 1024;
        let mut link_session = usb_link::UsbLinkSession::new(session_line).unwrap();
        let mut flash = Flash::<_, _, PICO_W_FLASH_BYTES>::new_blocking(p.FLASH);
        let mut flash_unique_id = [0_u8; 8];
        flash
            .blocking_unique_id(&mut flash_unique_id)
            .expect("Pico W flash must expose its physical unique identity");
        let bluetooth = async {
            cdc.wait_dtr().await;
            distributed_lenia::run(
                &spawner,
                &mut cdc,
                p.PIO0,
                p.DMA_CH0,
                p.DMA_CH1,
                p.PIN_23,
                p.PIN_24,
                p.PIN_25,
                p.PIN_29,
                &CYW43_FW,
                &CYW43_BTFW,
                &CYW43_NVRAM,
                CYW43_CLM,
                &runtime,
                flash_unique_id,
            )
            .await
        };
        let recovery = async {
            loop {
                let _ = bootsel::wait_for_request(&mut link_session).await;
            }
        };
        match select(bluetooth, recovery).await {
            Either::First(value) => value,
            Either::Second(value) => value,
        }
    }
}
