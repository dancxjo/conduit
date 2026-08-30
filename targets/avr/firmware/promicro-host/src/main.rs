#![no_std]
#![no_main]

use conduit_avr_promicro_host::provider;
use panic_halt as _;

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);

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

    loop {
        core::hint::spin_loop();
    }
}
