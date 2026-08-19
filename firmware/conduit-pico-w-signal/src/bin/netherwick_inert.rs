//! Inert first-flash qualification for the Netherwick carrier.
#![no_std]
#![no_main]

use core::fmt::Write as _;

use conduit_mpu6050::{
    I2cBaseAvailability, I2cProviderFailure, Mpu6050Failure, Mpu6050I2cProvider,
    Mpu6050Session, ALTERNATE_ADDRESS, DEFAULT_ADDRESS,
};
use embassy_executor::Executor;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{Blocking, Config as I2cConfig, I2c};
use embassy_rp::peripherals::{I2C1, PIN_2, PIN_3, USB};
use embassy_rp::usb;
use embassy_rp::Peri;
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::{Builder, Config, UsbDevice};
use heapless::String;
use static_cell::StaticCell;

struct NoAllocator;

unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 { core::ptr::null_mut() }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}

#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    // Match the working Brainstem failure disposition: leave both carrier
    // control outputs low and halt. Create UART is never initialized here.
    unsafe {
        core::ptr::write_volatile(0xd000_0018 as *mut u32, (1 << 18) | (1 << 19));
    }
    cortex_m::interrupt::disable();
    loop {
        cortex_m::asm::wfi();
    }
}

bind_interrupts!(struct Irqs { USBCTRL_IRQ => usb::InterruptHandler<USB>; });

static DEVICE: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL: StaticCell<[u8; 64]> = StaticCell::new();
static CDC: StaticCell<State> = StaticCell::new();

type BoardI2c = I2c<'static, I2C1, Blocking>;
type UsbDriver = usb::Driver<'static, USB>;
type InertUsbDevice = UsbDevice<'static, UsbDriver>;
type InertCdc = CdcAcmClass<'static, UsbDriver>;

struct Provider(BoardI2c);

impl Mpu6050I2cProvider for Provider {
    fn availability(&self) -> I2cBaseAvailability { I2cBaseAvailability::Available }
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2cProviderFailure> {
        self.0.blocking_write(address, bytes).map_err(|_| I2cProviderFailure::Write)
    }
    fn write_read(&mut self, address: u8, write: &[u8], read: &mut [u8]) -> Result<(), I2cProviderFailure> {
        self.0.blocking_write_read(address, write, read).map_err(|_| I2cProviderFailure::Read)
    }
}

fn usb_device(driver: usb::Driver<'static, USB>) -> (
    UsbDevice<'static, usb::Driver<'static, USB>>,
    CdcAcmClass<'static, usb::Driver<'static, USB>>,
) {
    let mut config = Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico W Netherwick Inert");
    // Keep the USB serial descriptor short.  RP2040 CDC configuration on the
    // target host fails after long static serial strings; this identifier is
    // still stable and product-specific while fitting the working path.
    config.serial_number = Some("nw-inert");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    let mut builder = Builder::new(
        driver, config, DEVICE.init([0; 256]), CONFIG.init([0; 256]),
        &mut [], CONTROL.init([0; 64]),
    );
    let class = CdcAcmClass::new(&mut builder, CDC.init(State::new()), 64);
    (builder.build(), class)
}

fn failure_name(failure: Mpu6050Failure) -> &'static str {
    match failure {
        Mpu6050Failure::InvalidAddress => "invalid_address",
        Mpu6050Failure::I2cBaseUnavailable => "i2c_base_unavailable",
        Mpu6050Failure::DeviceNoResponse => "device_no_response",
        Mpu6050Failure::IdentityMismatch { .. } => "identity_mismatch",
        Mpu6050Failure::WakeWriteFailed => "wake_write_failed",
        Mpu6050Failure::GyroConfigWriteFailed => "gyro_config_write_failed",
        Mpu6050Failure::AccelConfigWriteFailed => "accel_config_write_failed",
        Mpu6050Failure::FrameReadFailed => "frame_read_failed",
        Mpu6050Failure::ClockRegressed => "clock_regressed",
    }
}

async fn write_line(class: &mut CdcAcmClass<'static, usb::Driver<'static, USB>>, line: &str) {
    for chunk in line.as_bytes().chunks(64) {
        let _ = class.write_packet(chunk).await;
    }
}

#[embassy_executor::task]
async fn usb_device_task(mut device: InertUsbDevice) -> ! {
    device.run().await
}

#[embassy_executor::task]
async fn qualification_task(mut class: InertCdc, i2c1: Peri<'static, I2C1>, sda: Peri<'static, PIN_2>, scl: Peri<'static, PIN_3>) {
    class.wait_connection().await;
    // Keep the USB recovery/qualification entrance live before touching a
    // potentially absent or electrically held I²C bus. The probe is below
    // this boundary so a bus fault becomes an IMU receipt rather than
    // preventing CDC enumeration and remote BOOTSEL.
    // PIN_0, PIN_1 and UART0 are intentionally never taken.
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 100_000;
    let mut provider = Provider(I2c::new_blocking(i2c1, scl, sda, i2c_config));
    let mut result = Err(Mpu6050Failure::DeviceNoResponse);
    let mut address = DEFAULT_ADDRESS;
    for candidate in [DEFAULT_ADDRESS, ALTERNATE_ADDRESS] {
        let mut session = Mpu6050Session::new(candidate).unwrap();
        result = session.observe(&mut provider, 1);
        address = candidate;
        if result.is_ok() {
            // The first transaction proves identity and establishes the exact
            // configuration. Give the physical device a bounded wake interval,
            // then retain a distinct fresh frame as qualification evidence.
            Timer::after(Duration::from_millis(100)).await;
            result = session.observe(&mut provider, 2);
            break;
        }
    }
    write_line(&mut class, concat!("{\"schema\":\"conduit.netherwick/inert-boot@1\",\"build_id\":\"", env!("CONDUIT_NETHERWICK_INERT_BUILD_ID"), "\"}\n")).await;
    write_line(&mut class, "{\"schema\":\"conduit.netherwick/inert-disposition@1\",\"translator_oe\":\"low\",\"power_toggle\":\"low\",\"create_uart\":\"uninitialized\",\"i2c\":{\"controller\":1,\"sda_gpio\":2,\"scl_gpio\":3,\"hz\":100000}}\n").await;
    let mut line: String<384> = String::new();
    match result {
        Ok(sample) => { let _ = writeln!(line, "{{\"schema\":\"conduit.netherwick/imu-probe@1\",\"success\":true,\"address\":{},\"accel_mm_s2\":[{},{},{}],\"gyro_milliradians_s\":[{},{},{}]}}", address, sample.accel_x_mm_s2, sample.accel_y_mm_s2, sample.accel_z_mm_s2, sample.gyro_x_milliradians_s, sample.gyro_y_milliradians_s, sample.gyro_z_milliradians_s); }
        Err(failure) => { let _ = writeln!(line, "{{\"schema\":\"conduit.netherwick/imu-probe@1\",\"success\":false,\"address\":{},\"failure\":\"{}\"}}", address, failure_name(failure)); }
    }
    write_line(&mut class, &line).await;
    write_line(&mut class, "{\"schema\":\"conduit.netherwick/inert-terminal@1\",\"qualification_complete\":true,\"robot_control_ready\":false}\n").await;
    core::future::pending::<()>().await;
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());
    // These outputs are established before any peripheral probing. Ownership is
    // retained for the lifetime of the image so neither can float or be reused.
    let _power_toggle = Output::new(p.PIN_18, Level::Low);
    let _translator_oe = Output::new(p.PIN_19, Level::Low);
    let driver = usb::Driver::new(p.USB, Irqs);
    let (device, class) = usb_device(driver);
    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(usb_device_task(device).unwrap());
        spawner
            .spawn(qualification_task(class, p.I2C1, p.PIN_2, p.PIN_3).unwrap());
    });
}
