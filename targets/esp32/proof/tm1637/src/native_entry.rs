//! Native Root preparation. No Body birth, assignment installation or GPIO read.
use embassy_executor::Spawner;
use esp_hal::{
    clock::CpuClock,
    interrupt::software::SoftwareInterruptControl,
    timer::timg::TimerGroup,
    uart::{Config as UartConfig, Uart},
};
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 64 * 1024);
    crate::sdk_memory::initialize();
    let timer = TimerGroup::new(p.TIMG0);
    let interrupts = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timer.timer0, interrupts.software_interrupt0);
    let sensor = conduit_esp32_pulse_capture::Esp32PulseCapture::new(p.GPIO27);
    let seed = u64::from(esp_hal::rng::Rng::new().random());
    let radio = crate::network::start(spawner, p.WIFI, seed);
    let boot = crate::boot::ObservedBoot::observe().expect("initialized radio entropy required");
    let source = crate::native_source::load();
    let mut root = crate::native_root::NativeRoot::prepare(sensor, radio, &boot, source);
    root.start_preparation_service(spawner)
        .await
        .expect("Root preparation service must start");
    root.prepare_network_plan()
        .expect("current initialized Root advertisement");
    let uart = Uart::new(p.UART0, UartConfig::default())
        .expect("Root UART0 must initialize")
        .with_rx(p.GPIO3)
        .with_tx(p.GPIO1)
        .into_async();
    crate::native_control::serve(&mut root, uart)
        .await
        .expect("Root UART management failed");
}
