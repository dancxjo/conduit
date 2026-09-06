//! Pete capstone physical Play on the Pico W carrier.
#![no_std]
#![no_main]

use core::fmt::Write as _;

use aligned::{Aligned, A4};
use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder};
use embassy_executor::Executor;
use embassy_futures::select::{select, Either};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::InterruptHandler as I2cInterruptHandler;
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::usb;
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config, UsbDevice};
use heapless::String;
use static_cell::StaticCell;

#[path = "pete_capstone/create_acquisition.rs"]
mod create_acquisition;
#[path = "pete_capstone/create_battery_probe.rs"]
mod create_battery_probe;
#[path = "pete_capstone/create_control.rs"]
mod create_control;
#[path = "pete_capstone/create_full_stage.rs"]
mod create_full_stage;
#[path = "pete_capstone/create_lights_stage.rs"]
mod create_lights_stage;
#[path = "pete_capstone/create_link_gate.rs"]
mod create_link_gate;
#[path = "pete_capstone/create_listen.rs"]
mod create_listen;
#[path = "pete_capstone/create_motion.rs"]
mod create_motion;
#[path = "pete_capstone/create_play.rs"]
mod create_play;
#[path = "pete_capstone/create_power.rs"]
mod create_power;
#[path = "pete_capstone/create_presentation.rs"]
mod create_presentation;
#[path = "pete_capstone/imu_control.rs"]
mod imu_control;
#[path = "pete_capstone/pico_heartbeat.rs"]
mod pico_heartbeat;
#[path = "../radio.rs"]
mod radio;
#[path = "pete_capstone/uart_diagnostic.rs"]
mod uart_diagnostic;
// Compile the exact sealed capstone operations and fixed production-kernel
// topology from their canonical source.  The firmware must not grow a second,
// Pico-shaped scheduler or a lookalike copy of the portable Form.
#[path = "../../../../../../bodies/pete/src/proof/capstone_kernel.rs"]
mod capstone_kernel;
#[path = "../../../../../../bodies/pete/src/proof/capstone_operations.rs"]
mod capstone_operations;

struct NoAllocator;

unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // Match the working Brainstem failure disposition: disable the level
    // translator, release the Create power-toggle line, and halt. A Pico reset
    // then makes the Create supervisor reassert START/FULL, which also zeros
    // any prior wheel output before accepting another semantic request.
    unsafe {
        core::ptr::write_volatile(0xd000_0018 as *mut u32, (1 << 18) | (1 << 19));
    }
    cortex_m::interrupt::disable();
    loop {
        cortex_m::asm::wfi();
    }
}

bind_interrupts!(struct Irqs {
    I2C1_IRQ => I2cInterruptHandler<I2C1>;
});

static CYW43_FW: Aligned<A4, [u8; 231077]> = Aligned(*include_bytes!(
    "../../../assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/43439A0.bin"
));
static CYW43_NVRAM: Aligned<A4, [u8; 742]> = Aligned(*include_bytes!(
    "../../../assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/nvram_rp2040.bin"
));
const _CYW43_LICENSE: &[u8] = include_bytes!(
    "../../../assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab/LICENSE-permissive-binary-license-1.0.txt"
);

static DEVICE: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
static CDC: StaticCell<State> = StaticCell::new();

type UsbDriver = usb::Driver<'static, USB>;
type InertUsbDevice = UsbDevice<'static, UsbDriver>;
pub(crate) type InertCdc = CdcAcmClass<'static, UsbDriver>;

const BOOTSEL_QUERY: &[u8] = b"CONDUIT_BOOTSEL_QUERY@1";
const BOOTSEL_CHALLENGE_PREFIX: &str = "CONDUIT_BOOTSEL_CHALLENGE@1:";
const BOOTSEL_REQUEST_PREFIX: &str = "CONDUIT_REBOOT_BOOTSEL@1:";
const BOOTSEL_ACK: &[u8] = b"CONDUIT_REBOOT_BOOTSEL_ACK@1";
const UART_DIAGNOSTIC_PREFIX: &str = "CONDUIT_UART_DIAGNOSTIC@1:";
pub(crate) const BOOTSEL_FRAME_MAX: usize = 768;
const CONTROL_PACKET_WRITE_TIMEOUT_MS: u64 = 250;
const CAPSTONE_READY_WAIT_STEPS: usize = 200;
const BRINGUP_STAGE_IMU: u8 = 2;
const BRINGUP_STAGE_CREATE_FULL: u8 = 3;
const BRINGUP_STAGE_LIGHTS: u8 = 4;
const BRINGUP_STAGE_PRESENTATION: u8 = 5;
const BRINGUP_STAGE_MOTION: u8 = 6;
// This attended image admits only the bounded Create 1 music/light presentation
// and the lower no-motion diagnostic stages. Motion remains a distinct higher
// build stage and cannot be requested from this artifact.
const BRINGUP_STAGE: u8 = BRINGUP_STAGE_PRESENTATION;

fn usb_device(
    driver: usb::Driver<'static, USB>,
) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    CdcAcmClass<'static, usb::Driver<'static, USB>>,
) {
    let mut config = Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico W Pete Capstone");
    // Keep the USB serial descriptor short.  RP2040 CDC configuration on the
    // target host fails after long static serial strings; this identifier is
    // still stable and product-specific while fitting the working path.
    config.serial_number = Some("pete-capstone");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    let mut builder = Builder::new(
        driver,
        config,
        DEVICE.init([0; 256]),
        CONFIG.init([0; 256]),
        &mut [],
        CONTROL.init([0; 64]),
    );
    let class = CdcAcmClass::new(&mut builder, CDC.init(State::new()), 64);
    (builder.build(), class)
}

async fn write_line(class: &mut CdcAcmClass<'static, usb::Driver<'static, USB>>, line: &str) {
    for chunk in line.as_bytes().chunks(64) {
        let _ = class.write_packet(chunk).await;
    }
}

pub(crate) async fn send_control_frame(class: &mut InertCdc, payload: &[u8]) -> Result<(), ()> {
    let mut framed = [0_u8; BOOTSEL_FRAME_MAX + 2];
    let length = encode_stream_frame(payload, BOOTSEL_FRAME_MAX, &mut framed).map_err(|_| ())?;
    for chunk in framed[..length].chunks(64) {
        match select(
            class.write_packet(chunk),
            Timer::after(Duration::from_millis(CONTROL_PACKET_WRITE_TIMEOUT_MS)),
        )
        .await
        {
            Either::First(result) => result.map_err(|_| ())?,
            Either::Second(()) => return Err(()),
        }
    }
    if length % 64 == 0 {
        match select(
            class.write_packet(&[]),
            Timer::after(Duration::from_millis(CONTROL_PACKET_WRITE_TIMEOUT_MS)),
        )
        .await
        {
            Either::First(result) => result.map_err(|_| ())?,
            Either::Second(()) => return Err(()),
        }
    }
    Ok(())
}

async fn serve_conduit_services(class: &mut InertCdc) -> ! {
    let mut packet = [0_u8; 64];
    let mut request = [0_u8; BOOTSEL_FRAME_MAX];
    loop {
        // Scope the decoder read to one DTR-bearing CDC connection.  A host
        // close disables the endpoint; leave this scope and wait for the next
        // connection rather than trying to reuse a disabled endpoint.
        class.wait_connection().await;
        let mut decoder = match StreamFrameDecoder::<BOOTSEL_FRAME_MAX>::new(BOOTSEL_FRAME_MAX) {
            Ok(decoder) => decoder,
            Err(_) => core::future::pending::<StreamFrameDecoder<BOOTSEL_FRAME_MAX>>().await,
        };
        let request_length = 'connection: loop {
            match decoder.next_frame() {
                Ok(Some(frame)) => {
                    request[..frame.len()].copy_from_slice(frame);
                    break 'connection Some(frame.len());
                }
                Ok(None) => {}
                Err(_) => break 'connection None,
            }
            let read = match select(
                class.read_packet(&mut packet),
                Timer::after(Duration::from_millis(50)),
            )
            .await
            {
                Either::First(Ok(read)) => read,
                Either::First(Err(_)) => break 'connection None,
                Either::Second(()) if !class.dtr() => break 'connection None,
                Either::Second(()) => continue,
            };
            if decoder.accept_bytes(&packet[..read]).is_err() {
                break 'connection None;
            }
        };
        let Some(request_length) = request_length else {
            continue;
        };
        let request = &request[..request_length];
        if request == BOOTSEL_QUERY {
            let mut challenge = String::<BOOTSEL_FRAME_MAX>::new();
            if write!(
                challenge,
                "{BOOTSEL_CHALLENGE_PREFIX}{}",
                env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
            )
            .is_err()
                || send_control_frame(class, challenge.as_bytes())
                    .await
                    .is_err()
            {
                continue;
            }
            continue;
        }

        let mut expected = String::<BOOTSEL_FRAME_MAX>::new();
        if write!(
            expected,
            "{BOOTSEL_REQUEST_PREFIX}{}",
            env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
        )
        .is_err()
        {
            core::future::pending::<()>().await;
        }
        if request == expected.as_bytes() {
            if send_control_frame(class, BOOTSEL_ACK).await.is_err() {
                continue;
            }
            Timer::after(Duration::from_millis(100)).await;
            embassy_rp::rom_data::reset_to_usb_boot(0, 0);
            core::future::pending::<()>().await;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_MOTION && create_play::motion_request_matches(request) {
            create_play::serve_motion(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_CREATE_FULL && create_play::hello_request_matches(request)
        {
            create_play::serve_hello(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_CREATE_FULL && create_full_stage::request_matches(request)
        {
            create_full_stage::serve(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_LIGHTS && create_lights_stage::request_matches(request) {
            create_lights_stage::serve(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_PRESENTATION
            && create_presentation::request_matches(request)
        {
            create_presentation::serve(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_CREATE_FULL
            && create_battery_probe::request_matches(request)
        {
            create_battery_probe::serve(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_CREATE_FULL && create_listen::request_matches(request) {
            create_listen::serve(class).await;
            continue;
        }

        if BRINGUP_STAGE >= BRINGUP_STAGE_CREATE_FULL && create_power::request_matches(request) {
            create_power::serve(class).await;
            continue;
        }

        let mut diagnostic_request = String::<BOOTSEL_FRAME_MAX>::new();
        if write!(
            diagnostic_request,
            "{UART_DIAGNOSTIC_PREFIX}{}",
            env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
        )
        .is_err()
        {
            core::future::pending::<()>().await;
        }
        if request == diagnostic_request.as_bytes() {
            let snapshot = uart_diagnostic::snapshot();
            let create = create_control::snapshot();
            let translator_oe = if create.translator_enabled {
                "high"
            } else {
                "low"
            };
            let mut last_frame_hex = String::<60>::new();
            for byte in &snapshot.last_corrupt_frame[..snapshot.last_corrupt_frame_len] {
                let _ = write!(last_frame_hex, "{byte:02x}");
            }
            let first_byte_ms = snapshot.first_byte_ms.map(i64::from).unwrap_or(-1);
            let mut receipt = String::<768>::new();
            if write!(receipt, "{{\"schema\":\"conduit.pete/uart-diagnostic@1\",\"build_id\":\"{}\",\"window_start_ms\":{},\"window_end_ms\":{},\"oe_sequence\":\"low_until_attended_play\",\"translator_oe\":\"{}\",\"uart\":{{\"controller\":0,\"tx_gpio\":0,\"rx_gpio\":1,\"baud\":57600,\"data_bits\":8,\"stop_bits\":1,\"parity\":\"none\"}},\"rx_bytes\":{},\"tx_bytes\":{},\"valid_frames\":{},\"corrupt_frames\":{},\"resync_discarded_bytes\":{},\"timeouts\":{},\"errors\":{{\"overrun\":{},\"break\":{},\"parity\":{},\"framing\":{},\"other\":{}}},\"first_byte_after_boot_ms\":{},\"last_corrupt_frame\":{{\"present\":{},\"packet_id\":{},\"observed_len\":{},\"hex\":\"{}\"}}}}", env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"), snapshot.window_start_ms, embassy_time::Instant::now().as_millis() as u32, translator_oe, snapshot.rx_bytes, snapshot.tx_bytes, snapshot.valid_frames, snapshot.corrupt_frames, snapshot.resync_discarded_bytes, snapshot.timeouts, snapshot.overruns, snapshot.breaks, snapshot.parity_errors, snapshot.framing_errors, snapshot.other_errors, first_byte_ms, snapshot.corrupt_frames != 0, snapshot.last_corrupt_packet_id, snapshot.last_corrupt_frame_len, last_frame_hex).is_ok() {
                let _ = send_control_frame(class, receipt.as_bytes()).await;
            }
            continue;
        }
    }
}

#[embassy_executor::task]
async fn usb_device_task(mut device: InertUsbDevice) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn qualification_task(mut class: InertCdc, charging_indicator: Input<'static>) {
    class.wait_connection().await;
    // Emit immutable image identity before permitting any blocking peripheral
    // probe. A physically held I2C bus must not prevent this firmware from
    // enumerating and describing its exact running image.
    write_line(
        &mut class,
        concat!(
            "{\"schema\":\"conduit.pete/capstone-boot@1\",\"build_id\":\"",
            env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
            "\"}\n"
        ),
    )
    .await;
    if BRINGUP_STAGE >= BRINGUP_STAGE_IMU {
        imu_control::permit_probe_after_usb_identity();
    }
    // Stage one waits only for the independently initialized CYW43 heartbeat.
    // Later stages add their own evidence without weakening this first proof.
    for _ in 0..CAPSTONE_READY_WAIT_STEPS {
        let heartbeat_ready = pico_heartbeat::initialized();
        let imu_ready = BRINGUP_STAGE < BRINGUP_STAGE_IMU || imu_control::snapshot().samples >= 2;
        let create_ready = if BRINGUP_STAGE < BRINGUP_STAGE_CREATE_FULL {
            true
        } else {
            let create = create_control::snapshot();
            create.state == create_control::State::Full
                && create_control::is_fresh(
                    &create,
                    embassy_time::Instant::now().as_millis() as u32,
                )
        };
        if heartbeat_ready && imu_ready && create_ready {
            break;
        }
        Timer::after(Duration::from_millis(20)).await;
    }
    let charging_level = if charging_indicator.is_high() {
        "high"
    } else {
        "low"
    };
    let mut disposition: String<768> = String::new();
    let _ = writeln!(
        disposition,
        "{{\"schema\":\"conduit.pete/capstone-disposition@1\",\"bringup_stage\":{},\"translator_oe\":\"low\",\"power_toggle\":\"low\",\"create_uart\":\"isolated_no_tx\",\"charging_indicator\":{{\"gpio\":20,\"active_high\":true,\"level\":\"{}\"}},\"pico_led\":{{\"controller\":\"cyw43\",\"gpio\":0,\"heartbeat\":true,\"on_ms\":200,\"off_ms\":800,\"initialized\":{}}},\"i2c\":{{\"controller\":1,\"sda_gpio\":2,\"scl_gpio\":3,\"hz\":100000,\"enabled\":{}}},\"watchdog\":{{\"timeout_ms\":2000,\"feed_interval_ms\":250}}}}",
        BRINGUP_STAGE,
        charging_level,
        pico_heartbeat::initialized(),
        BRINGUP_STAGE >= BRINGUP_STAGE_IMU,
    );
    write_line(&mut class, &disposition).await;
    let imu = imu_control::snapshot();
    let imu_fresh = imu_control::is_fresh(&imu, embassy_time::Instant::now().as_millis() as u32);
    let mut line: String<512> = String::new();
    if BRINGUP_STAGE < BRINGUP_STAGE_IMU {
        let _ = writeln!(line, "{{\"schema\":\"conduit.pete/imu-probe@1\",\"success\":false,\"state\":\"staged-off\",\"address\":0,\"samples\":0,\"failure\":\"not-enabled-in-stage-1\"}}");
    } else if imu.samples >= 2 {
        let _ = writeln!(line, "{{\"schema\":\"conduit.pete/imu-probe@1\",\"success\":true,\"state\":\"{}\",\"address\":{},\"samples\":{},\"observed_at_ms\":{},\"fresh\":{},\"accel_mm_s2\":[{},{},{}],\"gyro_milliradians_s\":[{},{},{}],\"tilt_active\":{},\"impact_active\":{},\"calibration_generation\":{}}}", imu.state.name(), imu.address, imu.samples, imu.observed_at_ms, imu_fresh, imu.accel_x_mm_s2, imu.accel_y_mm_s2, imu.accel_z_mm_s2, imu.gyro_x_milliradians_s, imu.gyro_y_milliradians_s, imu.gyro_z_milliradians_s, imu.tilt_active, imu.impact_active, imu.calibration_generation);
    } else {
        let _ = writeln!(line, "{{\"schema\":\"conduit.pete/imu-probe@1\",\"success\":false,\"state\":\"{}\",\"address\":{},\"samples\":{},\"failure\":\"{}\"}}", imu.state.name(), imu.address, imu.samples, imu_control::failure_name(imu.failure));
    }
    write_line(&mut class, &line).await;
    let create = create_control::snapshot();
    let create_fresh =
        create_control::is_fresh(&create, embassy_time::Instant::now().as_millis() as u32);
    let create_ready = BRINGUP_STAGE >= BRINGUP_STAGE_CREATE_FULL
        && create.state == create_control::State::Full
        && create_fresh;
    let stage_complete = pico_heartbeat::initialized()
        && (BRINGUP_STAGE < BRINGUP_STAGE_IMU || imu.samples >= 2)
        && (BRINGUP_STAGE < BRINGUP_STAGE_CREATE_FULL || create_ready);
    let mut ready: String<512> = String::new();
    let _ = writeln!(
        ready,
        "{{\"schema\":\"conduit.pete/capstone-ready@1\",\"bringup_stage\":{},\"stage_complete\":{},\"qualification_complete\":false,\"robot_control_ready\":{},\"create_link_fresh\":{},\"create_packets\":{},\"ready_cue_command_sent\":{},\"form\":\"pete-capstone\",\"kernel\":\"conduit-kernel\",\"oi_exposed\":false}}",
        BRINGUP_STAGE,
        stage_complete,
        create_ready,
        create_fresh,
        create.packets,
        create_control::ready_cue_command_sent(),
    );
    write_line(&mut class, &ready).await;
    serve_conduit_services(&mut class).await;
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    // These outputs are established before any peripheral probing. Ownership is
    // retained for the lifetime of the image so neither can float or be reused.
    let power_toggle = Output::new(p.PIN_18, Level::Low);
    let translator_oe = Output::new(p.PIN_19, Level::Low);
    let charging_indicator = Input::new(p.PIN_20, Pull::Down);
    let driver = usb::Driver::new(p.USB, radio::UsbIrq);
    let (device, class) = usb_device(driver);
    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(usb_device_task(device).unwrap());
        spawner.spawn(
            create_control::task(
                p.UART0,
                p.PIN_0,
                p.PIN_1,
                power_toggle,
                translator_oe,
                p.WATCHDOG,
            )
            .unwrap(),
        );
        if BRINGUP_STAGE >= BRINGUP_STAGE_IMU {
            spawner.spawn(imu_control::task(p.I2C1, p.PIN_2, p.PIN_3).unwrap());
        }
        spawner.spawn(
            pico_heartbeat::task(
                spawner,
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
            .unwrap(),
        );
        spawner.spawn(qualification_task(class, charging_indicator).unwrap());
    });
}
