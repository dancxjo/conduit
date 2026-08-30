#![no_std]
#![no_main]

use conduit_avr_promicro_host::{
    boot::BootIdentity,
    rx_diagnostic::{
        RxDiagnosticEvidence, RX_DIAGNOSTIC_DURATION_US, RX_DIAGNOSTIC_RECEIPT_BYTES,
        RX_DIAGNOSTIC_REQUEST, RX_DIAGNOSTIC_SAMPLES,
    },
    usb_line::UsbLine,
};
use panic_halt as _;

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    // Explicitly restore USART1 to its reset state. This image never creates a
    // serial adapter, so TXEN1 remains clear for the entire diagnostic.
    peripherals.USART1.ucsr1b.reset();
    let pins = arduino_hal::pins!(peripherals);
    let _boot = BootIdentity::acquire(arduino_hal::Eeprom::new(peripherals.EEPROM));
    let rx = pins.rx.into_floating_input();
    let _tx = pins.tx.into_floating_input();
    let _power_toggle = pins.d4.into_floating_input();
    let _charging = pins.d5.into_floating_input();
    let mut host_line = UsbLine::new_receive_only(peripherals.USB_DEVICE, peripherals.PLL);
    let mut output = [0_u8; RX_DIAGNOSTIC_RECEIPT_BYTES];
    let mut output_len = 0_usize;
    let mut output_offset = 0_usize;
    let mut sampled = false;

    loop {
        host_line.poll();
        if output_offset < output_len {
            if let Ok(written) = host_line.write(&output[output_offset..output_len]) {
                output_offset += written;
            }
            continue;
        }
        if sampled {
            continue;
        }
        let mut request = [0_u8; RX_DIAGNOSTIC_REQUEST.len()];
        if host_line.read(&mut request) != Ok(RX_DIAGNOSTIC_REQUEST.len())
            || &request != RX_DIAGNOSTIC_REQUEST
        {
            continue;
        }

        let mut evidence = RxDiagnosticEvidence::new();
        let mut previous = None;
        for _ in 0..RX_DIAGNOSTIC_SAMPLES {
            let high = rx.is_high();
            evidence.push(high, previous);
            previous = Some(high);
            arduino_hal::delay_us(RX_DIAGNOSTIC_DURATION_US / u32::from(RX_DIAGNOSTIC_SAMPLES));
        }
        output = evidence.encode();
        output_len = output.len();
        sampled = true;
    }
}
