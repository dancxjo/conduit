//! Composition root for the attended radio/sensor/display integration bench.
use conduit_esp32_network_switch::AccessPointServices;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock, interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup,
};
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 96 * 1024);
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 88 * 1024);
    let timer = TimerGroup::new(p.TIMG0);
    let interrupts = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timer.timer0, interrupts.software_interrupt0);
    let mut sensor = conduit_esp32_pulse_capture::Esp32PulseCapture::new(p.GPIO27);
    let mut pins = crate::electrical::DisplayPins::new(p.GPIO18, p.GPIO19);
    let mut live = crate::live::Live::prepare();
    let seed = u64::from(esp_hal::rng::Rng::new().random());
    let crate::network::Radio {
        controller: _controller,
        stacks: _stacks,
        ap_address: _ap_address,
        ap_config: _ap_config,
    } = crate::network::start(spawner, p.WIFI, seed);
    let boot = crate::boot::ObservedBoot::observe().expect("radio entropy must be initialized");
    boot.print();
    let mut services = crate::services::control(true);
    services.restore().await.unwrap();
    conduit_environment_http::set_ap_ready();
    esp_println::println!(
        "AP Conduit-Climate http://192.168.4.1/ heap free={}",
        esp_alloc::HEAP.free()
    );
    let mut sample_tick = 0u8;
    loop {
        if sample_tick == 0 {
            live.sample(&mut sensor);
        }
        live.display(&mut pins);
        sample_tick = (sample_tick + 1) % 5;
        Timer::after(Duration::from_secs(3)).await;
    }
}
