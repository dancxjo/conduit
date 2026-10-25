//! Initialize actual Root resources and install an optional linked Body.
//! Plot planning, effect authority, and execution remain separate admission.
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
    esp_alloc::heap_allocator!(size: crate::sdk_memory::ROOT_INTERNAL_BYTES);
    let sdk_reservation = crate::sdk_memory::initialize();
    let timer = TimerGroup::new(p.TIMG0);
    let interrupts = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timer.timer0, interrupts.software_interrupt0);
    let sensor = conduit_esp32_pulse_capture::Esp32PulseCapture::new(p.GPIO27);
    let seed = u64::from(esp_hal::rng::Rng::new().random());
    let radio = crate::network::start(spawner, p.WIFI, seed);
    // Boot observation is construction scratch. Root and each initialized
    // provider retain their owned exact IDs before the first await; do not
    // retain a second copy in the permanent main task frame.
    let mut root = {
        let boot =
            crate::boot::ObservedBoot::observe().expect("initialized radio entropy required");
        // SAFETY: the actual TimerGroup was consumed by esp_rtos::start above.
        // Root retains one pending software-timer slot; Play admission is separate.
        let wait = unsafe { crate::native_wait::prepare(&boot) }
            .expect("initialized monotonic timer slot");
        let display = crate::native_display::NativeDisplay::prepare(p.GPIO18, p.GPIO19, &boot)
            .expect("initialized Root display provider");
        let source = crate::native_source::load();
        crate::native_root::NativeRoot::prepare(
            sensor,
            radio,
            &boot,
            source,
            sdk_reservation,
            wait,
            display,
        )
    };
    #[cfg(feature = "body-image-root")]
    root.install_linked_body().expect("linked body biography");
    root.start_preparation_service(spawner)
        .await
        .expect("Root preparation service must start");
    root.prepare_network_plan()
        .expect("current initialized Root advertisement");
    #[cfg(feature = "target-memory-check")]
    root.measure_local_sampler().expect("target sampler measurement");
    let uart = Uart::new(p.UART0, UartConfig::default())
        .expect("Root UART0 must initialize")
        .with_rx(p.GPIO3)
        .with_tx(p.GPIO1)
        .into_async();
    crate::native_control::serve(&mut root, uart)
        .await
        .expect("Root UART management failed");
}
