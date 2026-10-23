#![no_std]
#![no_main]
//! Attended bench plus separately selected native Root preparation mode.
#[cfg(feature = "native-root")]
extern crate alloc;
#[cfg(not(feature = "live"))]
use conduit_tm1637_wire::{transmit, Packet};
use esp_backtrace as _;
#[cfg(not(feature = "ap"))]
use esp_hal::{delay::Delay, main};
#[cfg(feature = "ap")]
mod boot;
#[cfg(feature = "native-root")]
mod native_command;
#[cfg(feature = "native-root")]
mod native_control;
#[cfg(feature = "native-root")]
mod native_credentials;
#[cfg(feature = "native-root")]
mod native_entry;
#[cfg(feature = "native-root")]
mod native_root;
#[cfg(feature = "ap")]
mod network;
#[cfg(all(feature = "ap", not(feature = "native-root")))]
mod network_entry;
#[cfg(feature = "ap")]
mod sdk_memory;
#[cfg(feature = "ap")]
mod services;
// Reuse the existing allocation-free appliance service bench and DHCP grammar.
#[cfg(all(feature = "capture", not(feature = "native-root")))]
mod capture;
#[cfg(not(feature = "native-root"))]
mod electrical;
#[cfg(feature = "ap")]
#[path = "../../../../rp2040/proof/dht11/src/http_ap.rs"]
mod http_ap;
#[cfg(all(feature = "live", not(feature = "native-root")))]
mod live;
esp_bootloader_esp_idf::esp_app_desc!();
#[cfg(not(feature = "ap"))]
#[main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default());
    let delay = Delay::new();
    #[cfg(feature = "live")]
    {
        esp_alloc::heap_allocator!(size: 96 * 1024);
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 88 * 1024);
    }

    #[cfg(feature = "capture")]
    let mut sensor = conduit_esp32_pulse_capture::Esp32PulseCapture::new(p.GPIO27);
    let mut pins = electrical::DisplayPins::new(p.GPIO18, p.GPIO19);
    delay.delay_millis(3000);
    esp_println::println!("{{\"event\":\"display-probe\",\"fixture\":true,\"clk_gpio\":18,\"dio_gpio\":19,\"supply\":\"3V3\"}}");
    pins.check_drive();
    #[cfg(feature = "live")]
    {
        let mut live = live::Live::prepare();
        let mut sample_tick = 0u8;
        loop {
            if sample_tick == 0 {
                live.sample(&mut sensor);
            }
            live.display(&mut pins);
            sample_tick = (sample_tick + 1) % 5;
            delay.delay_millis(3_000);
        }
    }
    #[cfg(not(feature = "live"))]
    loop {
        #[cfg(feature = "capture")]
        capture::probe(&mut sensor);
        for (label, registers) in [
            ("all-segments", [0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
            ("1234", [0x06, 0x5b, 0x4f, 0x66, 0, 0]),
            ("67.6F-fixture", [0x7d, 0x87, 0x7d, 0x71, 0, 0]),
        ] {
            let (clock, data) = pins.levels();
            esp_println::println!(
                "{{\"event\":\"display-idle\",\"clock_high\":{},\"data_high\":{}}}",
                clock,
                data
            );
            let data = [
                0xc0,
                registers[0],
                registers[1],
                registers[2],
                registers[3],
                registers[4],
                registers[5],
            ];
            let result = transmit(
                &mut pins,
                &[
                    Packet { bytes: &[0x40] },
                    Packet { bytes: &data },
                    Packet { bytes: &[0x8b] },
                ],
                || false,
            );
            esp_println::println!(
                "{{\"event\":\"display-write\",\"fixture_text\":\"{}\",\"result\":\"{:?}\"}}",
                label,
                result
            );
            delay.delay_millis(3000);
        }
    }
}
