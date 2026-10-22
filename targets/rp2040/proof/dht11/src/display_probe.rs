//! Attended electrical fixture, not an installed Body or a sensor reading.
use crate::{bootsel, usb_link::UsbLinkSession};
use conduit_rp2040_tm1637::PicoTm1637;
use conduit_tm1637_wire::{transmit, Packet};
use core::fmt::Write;
use embassy_rp::{gpio::Pin, Peri};
use embassy_time::{Duration, Timer};

pub async fn run(
    clock: Peri<'static, impl Pin>,
    data: Peri<'static, impl Pin>,
    link: &mut UsbLinkSession,
) {
    let fault = crate::kernel_fault::take();
    let mut pins = PicoTm1637::new(clock, data);
    link.wait_connection().await;
    while !link.class.dtr() {
        Timer::after(Duration::from_millis(25)).await;
    }
    Timer::after(Duration::from_secs(2)).await;
    let (clock_high, data_high) = pins.levels();
    let mut levels = heapless::String::<128>::new();
    let _ = writeln!(levels, "{{\"event\":\"display-idle-levels\",\"clock_high\":{clock_high},\"data_high\":{data_high}}}");
    let _ = link.write_text(&levels).await;
    if let Some(fault) = fault {
        let mut line = heapless::String::<256>::new();
        let _ = writeln!(line, "{{\"event\":\"recovered-display-fault\",\"kind\":{},\"line_or_pc\":{},\"column_or_lr\":{}}}", fault[1], fault[6], fault[7]);
        let _ = link.write_text(&line).await;
        loop {
            let _ = bootsel::wait_for_request(link).await;
        }
    }

    let _ = link.write_text("{\"event\":\"display-probe\",\"clk_gpio\":3,\"dio_gpio\":4,\"enable_gpio\":null,\"brightness\":2,\"fixture\":true}\n").await;
    // Reviewed electrical test vectors. These are not a semantic Face, Mask
    // output, measured temperature, or a fork of the Source packet encoder.
    for (label, registers) in [
        ("8888", [0x7f, 0x7f, 0x7f, 0x7f, 0, 0]),
        ("1234", [0x06, 0x5b, 0x4f, 0x66, 0, 0]),
    ] {
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
                Packet { bytes: &[0x8a] },
            ],
            || false,
        );
        let mut line = heapless::String::<256>::new();
        match result {
            Ok(()) => {
                let _ = writeln!(line, "{{\"event\":\"display-write\",\"fixture_text\":\"{label}\",\"status\":\"acknowledged\",\"acknowledged_bytes\":9}}");
            }
            Err(ref error) => {
                let _ = writeln!(line, "{{\"event\":\"display-write\",\"fixture_text\":\"{label}\",\"status\":\"failed\",\"reason\":\"{error:?}\"}}");
            }
        }
        let _ = link.write_text(&line).await;
        if result.is_err() {
            break;
        }
        Timer::after(Duration::from_secs(2)).await;
    }
    drop(pins);
    let _ = link
        .write_text("{\"event\":\"display-drained\",\"visual_confirmation\":false}\n")
        .await;
    loop {
        let _ = bootsel::wait_for_request(link).await;
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    let location = info.location();
    crate::kernel_fault::record(
        3,
        0,
        0,
        0,
        0,
        location.map_or(0, |l| l.line()),
        location.map_or(0, |l| l.column()),
    );
    reboot()
}
#[cortex_m_rt::exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    crate::kernel_fault::record(4, 0, 0, 0, 0, frame.pc(), frame.lr());
    reboot()
}
fn reboot() -> ! {
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    loop {
        cortex_m::asm::wfi();
    }
}
