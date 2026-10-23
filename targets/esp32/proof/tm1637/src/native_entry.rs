//! Native Root preparation. No Body birth, assignment installation or GPIO read.
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
    let sensor = conduit_esp32_pulse_capture::Esp32PulseCapture::new(p.GPIO27);
    let seed = u64::from(esp_hal::rng::Rng::new().random());
    let radio = crate::network::start(spawner, p.WIFI, seed);
    let boot = crate::boot::ObservedBoot::observe().expect("initialized radio entropy required");
    boot.print();
    let root = crate::native_root::NativeRoot::prepare(sensor, radio, &boot);
    let mut output = [0; 8192];
    let length = root.write_offer(&mut output).expect("bounded native offer");
    esp_println::println!("{{\"event\":\"native-root-offer\",\"body_installed\":false,\"native_plan_installed\":false,\"image_sha256\":null,\"advertisement\":{}}}", core::str::from_utf8(&output[..length]).unwrap());
    loop {
        // Retain initialized ownership and the independently running interfaces.
        // UART management/Body admission is not fabricated by this preparation.
        Timer::after(Duration::from_secs(30)).await;
        let length = root
            .write_offer(&mut output)
            .expect("retained native offer");
        esp_println::println!("{{\"event\":\"native-root-offer\",\"body_installed\":false,\"native_plan_installed\":false,\"image_sha256\":null,\"advertisement\":{}}}", core::str::from_utf8(&output[..length]).unwrap());
    }
}
