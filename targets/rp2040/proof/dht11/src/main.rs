//! Finite DHT11 electrical diagnostic. This is not the logger or a Conduit Host.
//! Features select finite electrical, Source, or production-kernel bench proof.
#![no_std]
#![no_main]

#[cfg(all(not(feature = "kernel-evaluation"), not(feature = "display-wire")))]
use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_futures::join::join;
#[cfg(all(not(feature = "pio-capture"), not(feature = "display-wire")))]
use embassy_rp::gpio::{Flex, Pull};
use embassy_rp::{bind_interrupts, peripherals::USB, usb};
#[cfg(all(not(feature = "kernel-evaluation"), not(feature = "display-wire")))]
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder, Config,
};
#[cfg(all(not(feature = "kernel-evaluation"), not(feature = "display-wire")))]
use panic_halt as _;

#[cfg(feature = "display-wire")]
mod display_probe;
#[cfg(feature = "http-ap")]
mod http_ap;
#[cfg(feature = "http-ap")]
mod http_radio;
#[cfg(feature = "http-ap")]
use conduit_environment_http as http_view;
mod usb_link;
// Reuse the exact lifecycle challenge/ACK and reboot implementation.
#[path = "../../../firmware/pico-w-signal/src/bootsel.rs"]
#[allow(
    unexpected_cfgs,
    reason = "shared lifecycle module's other firmware modes are absent here"
)]
mod bootsel;
mod signal_image {
    pub const FIRMWARE_BUILD_ID: &str = env!("CONDUIT_DHT11_BUILD_ID");
}
#[cfg(feature = "kernel-evaluation")]
mod kernel_authority;
#[cfg(feature = "kernel-evaluation")]
mod kernel_back;
#[cfg(any(feature = "kernel-evaluation", feature = "display-wire"))]
mod kernel_fault;
#[cfg(feature = "kernel-evaluation")]
mod kernel_manifest;
#[cfg(feature = "kernel-evaluation")]
mod kernel_probe;
#[cfg(feature = "kernel-evaluation")]
mod kernel_report;
#[cfg(feature = "source-evaluation")]
mod probe_heap;
#[cfg(all(feature = "source-evaluation", not(feature = "kernel-evaluation")))]
mod source;

#[cfg(not(feature = "source-evaluation"))]
struct NoAllocator;
#[cfg(not(feature = "source-evaluation"))]
unsafe impl core::alloc::GlobalAlloc for NoAllocator {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
}
#[cfg(not(feature = "source-evaluation"))]
#[global_allocator]
static ALLOCATOR: NoAllocator = NoAllocator;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

#[cfg(all(feature = "pio-capture", not(feature = "http-ap")))]
bind_interrupts!(struct CaptureIrqs {
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO1>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH2>;
});

#[cfg(feature = "http-ap")]
bind_interrupts!(struct CaptureIrqs {
    PIO1_IRQ_0 => embassy_rp::pio::InterruptHandler<embassy_rp::peripherals::PIO1>;
    DMA_IRQ_0 => embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH0>, embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH1>, embassy_rp::dma::InterruptHandler<embassy_rp::peripherals::DMA_CH2>;
});

#[cfg(all(not(feature = "pio-capture"), not(feature = "display-wire")))]
fn wait_level(pin: &Flex<'_>, high: bool, maximum_us: u64) -> Result<(), &'static str> {
    let start = Instant::now().as_micros();
    while pin.is_high() != high {
        if Instant::now().as_micros() - start >= maximum_us {
            return Err("edge-timeout");
        }
    }
    Ok(())
}

#[cfg(all(not(feature = "pio-capture"), not(feature = "display-wire")))]
fn pulse(pin: &Flex<'_>, high: bool, maximum_us: u64) -> Result<u64, &'static str> {
    let start = Instant::now().as_micros();
    while pin.is_high() == high {
        if Instant::now().as_micros() - start >= maximum_us {
            return Err("pulse-timeout");
        }
    }
    Ok(Instant::now().as_micros() - start)
}

#[cfg(all(not(feature = "pio-capture"), not(feature = "display-wire")))]
fn capture(pin: &Flex<'_>, bytes: &mut [u8; 5]) -> Result<(), &'static str> {
    // Observe the released bus before looking for the sensor's response.
    wait_level(pin, true, 100)?;
    wait_level(pin, false, 150)?;
    let low = pulse(pin, false, 150)?;
    let high = pulse(pin, true, 150)?;
    if !(55..=110).contains(&low) || !(55..=110).contains(&high) {
        return Err("response-timing");
    }
    for bit in 0..40 {
        let low = pulse(pin, false, 120)?;
        let high = pulse(pin, true, 120)?;
        if !(30..=85).contains(&low) || !(15..=95).contains(&high) || (40..55).contains(&high) {
            return Err("bit-timing");
        }
        bytes[bit / 8] = (bytes[bit / 8] << 1) | u8::from(high >= 55);
    }
    Ok(())
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    #[cfg(feature = "source-evaluation")]
    unsafe {
        probe_heap::ALLOCATOR.initialize();
    }
    let p = embassy_rp::init(Default::default());
    #[cfg(feature = "http-ap")]
    let network = http_radio::Resources {
        spawner: _spawner,
        pio: p.PIO0,
        tx: p.DMA_CH0,
        rx: p.DMA_CH1,
        power: p.PIN_23,
        data: p.PIN_24,
        cs: p.PIN_25,
        clock: p.PIN_29,
    };
    #[cfg(all(not(feature = "pio-capture"), not(feature = "display-wire")))]
    let mut pin = {
        let mut pin = Flex::new(p.PIN_2);
        pin.set_as_input();
        pin.set_pull(Pull::Up);
        pin
    };
    #[cfg(feature = "pio-capture")]
    let mut pio = embassy_rp::pio::Pio::new(p.PIO1, CaptureIrqs);
    #[cfg(feature = "pio-capture")]
    let program = conduit_rp2040_pulse_capture::PulseProgram::load(&mut pio.common);
    #[cfg(feature = "pio-capture")]
    let electrical = conduit_rp2040_pulse_capture::PicoPulseCapture::new(
        &mut pio.common,
        pio.sm0,
        embassy_rp::dma::Channel::new(p.DMA_CH2, CaptureIrqs),
        p.PIN_2,
        program,
    );
    let mut config = Config::new(0x2e8a, 0x000a);
    config.manufacturer = Some("Conduit");
    config.product = Some("Pico DHT11 Probe");
    config.serial_number = Some("conduit-pico-dht11-probe");
    config.max_packet_size_0 = 64;
    config.max_power = 100;
    static DEVICE: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static CONFIG: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static BOS: static_cell::StaticCell<[u8; 256]> = static_cell::StaticCell::new();
    static CONTROL: static_cell::StaticCell<[u8; 64]> = static_cell::StaticCell::new();
    static STATE: static_cell::StaticCell<State> = static_cell::StaticCell::new();
    let mut builder = Builder::new(
        usb::Driver::new(p.USB, Irqs),
        config,
        DEVICE.init([0; 256]),
        CONFIG.init([0; 256]),
        BOS.init([0; 256]),
        CONTROL.init([0; 64]),
    );
    let serial = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);
    let mut link = usb_link::UsbLinkSession::new(serial);
    let mut device = builder.build();
    #[cfg(all(not(feature = "kernel-evaluation"), not(feature = "display-wire")))]
    let diagnostic = async {
        #[cfg(feature = "pio-capture")]
        let mut electrical = electrical;
        // Physical proof is attended; the production logger must not wait here.
        link.wait_connection().await;
        while !link.class.dtr() {
            Timer::after(Duration::from_millis(25)).await;
        }
        Timer::after(Duration::from_secs(2)).await;
        #[cfg(feature = "source-evaluation")]
        let mut decoder = {
            let mut decoder = source::SourceDecoder::prepare();
            probe_heap::ALLOCATOR.seal();
            decoder.check_adapter();
            let (peak, retained, allocations) = probe_heap::ALLOCATOR.observation();
            let mut line = heapless::String::<768>::new();
            let _ = writeln!(line, "{{\"schema\":\"conduit.dht11-source-probe/v1\",\"status\":\"prepared\",\"source_sha256\":\"{}\",\"heap_capacity_bytes\":{},\"heap_peak_bytes\":{peak},\"heap_retained_bytes\":{retained},\"preparation_allocations\":{allocations},\"allocator_sealed\":true,\"target_adapter_checks_passed\":5,\"capture_owner\":\"{}\",\"capture_source_sha256\":\"{}\"}}", env!("CONDUIT_DHT11_SOURCE_SHA256"), probe_heap::CAPACITY, if cfg!(feature = "pio-capture") { "pio-dma" } else { "interrupt-masked-proof" }, env!("CONDUIT_DHT11_CAPTURE_SOURCE_SHA256"));
            let _ = link.write_text(&line).await;
            decoder
        };
        for sequence in 0..32_u8 {
            let captured_at_ms = Instant::now().as_millis();
            let mut wire = [0; 5];
            #[cfg(all(not(feature = "pio-capture"), not(feature = "display-wire")))]
            let outcome = {
                pin.set_low();
                pin.set_as_output();
                Timer::after(Duration::from_millis(20)).await;
                // Legacy electrical diagnostic only; the PIO path leaves IRQs on.
                let outcome = cortex_m::interrupt::free(|_| {
                    pin.set_as_input();
                    capture(&pin, &mut wire)
                });
                pin.set_as_input();
                outcome
            };
            #[cfg(feature = "pio-capture")]
            let outcome = match electrical.read(decoder.pulse_request).await {
                Ok(capture)
                    if capture.disposition
                        == conduit_pulse_capture::PulseReadDisposition::Complete
                        && capture.length == 5 =>
                {
                    wire.copy_from_slice(&capture.wire[..5]);
                    Ok(())
                }
                Ok(capture)
                    if capture.disposition
                        == conduit_pulse_capture::PulseReadDisposition::ProviderFailed =>
                {
                    Err("capture-provider-failed")
                }
                Ok(_) => Err("capture-timeout"),
                Err(_) => Err("capture-geometry-refused"),
            };
            let mut line = heapless::String::<384>::new();
            match outcome {
                Ok(()) => {
                    #[cfg(not(feature = "source-evaluation"))]
                    let _ = writeln!(line, "{{\"schema\":\"conduit.dht11-probe/v1\",\"sequence\":{sequence},\"captured_at_ms\":{captured_at_ms},\"gpio\":2,\"status\":\"captured\",\"wire\":[{},{},{},{},{}]}}", wire[0],wire[1],wire[2],wire[3],wire[4]);
                    #[cfg(feature = "source-evaluation")]
                    match decoder.evaluate(5, wire) {
                        Ok((temperature, humidity)) => {
                            let _ = writeln!(line, "{{\"schema\":\"conduit.dht11-source-probe/v1\",\"sequence\":{sequence},\"captured_at_ms\":{captured_at_ms},\"gpio\":2,\"wire\":[{},{},{},{},{}],\"status\":\"observation\",\"temperature\":{{\"type\":\"Temperature\",\"coefficient\":{},\"exponent\":{},\"unit\":\"°C\"}},\"humidity\":{{\"type\":\"Ratio\",\"coefficient\":{},\"exponent\":{},\"unit\":\"%\"}}}}", wire[0],wire[1],wire[2],wire[3],wire[4], temperature.coefficient(), temperature.exponent(), humidity.coefficient(), humidity.exponent());
                        }
                        Err(reason) => {
                            let _ = writeln!(line, "{{\"schema\":\"conduit.dht11-source-probe/v1\",\"sequence\":{sequence},\"status\":\"unavailable\",\"reason\":\"{reason}\",\"wire\":[{},{},{},{},{}]}}", wire[0],wire[1],wire[2],wire[3],wire[4]);
                        }
                    }
                }
                Err(error) => {
                    let schema = if cfg!(feature = "source-evaluation") {
                        "conduit.dht11-source-probe/v1"
                    } else {
                        "conduit.dht11-probe/v1"
                    };
                    let _ = writeln!(line, "{{\"schema\":\"{schema}\",\"sequence\":{sequence},\"captured_at_ms\":{captured_at_ms},\"gpio\":2,\"status\":\"{error}\"}}");
                }
            }
            let _ = link.write_text(&line).await;
            Timer::after(Duration::from_secs(3)).await;
        }
        loop {
            let _ = bootsel::wait_for_request(&mut link).await;
        }
    };
    #[cfg(all(feature = "kernel-evaluation", not(feature = "http-ap")))]
    let diagnostic = kernel_probe::run(electrical, &mut link);
    #[cfg(feature = "http-ap")]
    let diagnostic = kernel_probe::run(electrical, &mut link, network);
    #[cfg(feature = "display-wire")]
    let diagnostic = display_probe::run(p.PIN_3, p.PIN_4, &mut link);
    join(device.run(), diagnostic).await;
}
