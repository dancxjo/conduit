#![no_std]
#![no_main]

use conduit_assigned_plan::AssignedIdentity;
use conduit_avr_promicro_host::{
    assigned_receiver::{AssignedReceiver, HOST_ID},
    boot::BootIdentity,
    provider,
    usb_line::UsbLine,
};
use panic_halt as _;

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);
    let boot = BootIdentity::acquire(arduino_hal::Eeprom::new(peripherals.EEPROM));

    // Exact board mechanism: Create cargo pin 3 is D4. It remains high
    // impedance until an admitted power operation is dispatched.
    let _power_toggle = pins.d4.into_floating_input();
    // Exact board mechanism: Create cargo pin 13 is observed on D5.
    let _charging = pins.d5.into_floating_input();

    // Construct the shared Create provider at the required 57,600 8N1. The
    // provider does not transmit merely because it exists; ordinary assigned
    // Host operation dispatch is the only future caller allowed to write.
    let create_uart = arduino_hal::default_serial!(peripherals, pins, 57_600);
    let _create = provider::AvrCreateUart::new(create_uart);
    let mut host_line = UsbLine::new(peripherals.USB_DEVICE, peripherals.PLL, boot.usb_serial);
    let host = AssignedIdentity::from_text(HOST_ID);
    let mut assigned = AssignedReceiver::new();

    loop {
        if host_line.poll() {
            let mut incoming = [0_u8; 32];
            if let Ok(length) = host_line.read(&mut incoming) {
                if length != 0 {
                    match assigned.push(&incoming[..length]) {
                        Ok(Some(_)) => {
                            let _validated = assigned.validate(host, boot.assigned);
                        }
                        Ok(None) => {}
                        Err(_refusal) => {}
                    }
                }
            }
        }
        core::hint::spin_loop();
    }
}
