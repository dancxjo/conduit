#![no_std]
#![no_main]

use conduit_assigned_plan::{
    encode_assigned_execution_receipt, ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES,
};
use conduit_avr_promicro_host::{
    activation_receiver::ActivationReceiver,
    assigned_receiver::{AssignedReceiver, HOST_IDENTITY},
    boot::BootIdentity,
    executor::execute_contact,
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
    let mut create = provider::AvrCreateUart::new(create_uart);
    let mut host_line = UsbLine::new(peripherals.USB_DEVICE, peripherals.PLL, boot.usb_serial);
    let host = HOST_IDENTITY;
    let mut assigned = AssignedReceiver::new();
    let mut activation = ActivationReceiver::new();
    let mut plan = None;
    let mut output = [0_u8; ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES + 1];
    let mut output_len = 0_usize;
    let mut output_offset = 0_usize;

    loop {
        host_line.poll();
        if output_offset < output_len {
            let end = (output_offset + 32).min(output_len);
            if let Ok(written) = host_line.write(&output[output_offset..end]) {
                output_offset += written;
                if output_offset == output_len {
                    output_len = 0;
                    output_offset = 0;
                    plan = None;
                    assigned.reset();
                    activation.reset();
                }
            }
            continue;
        }
        let mut incoming = [0_u8; 32];
        if let Ok(length) = host_line.read(&mut incoming) {
            if length != 0 {
                if plan.is_none() {
                    match assigned.push(&incoming[..length]) {
                        Ok(Some(_)) => match assigned.validate(host, boot.assigned) {
                            Ok(validated) => plan = Some(validated),
                            Err(_) => assigned.reset(),
                        },
                        Ok(None) => {}
                        Err(_) => assigned.reset(),
                    }
                } else {
                    match activation.push(&incoming[..length]) {
                        Ok(Some(active)) => {
                            let mut value = [0_u8; 1];
                            let receipt = match plan {
                                Some(validated) => execute_contact(
                                    validated,
                                    active,
                                    &mut create,
                                    2_000,
                                    &mut value,
                                ),
                                None => {
                                    plan = None;
                                    assigned.reset();
                                    activation.reset();
                                    continue;
                                }
                            };
                            if let Ok(length) =
                                encode_assigned_execution_receipt(receipt, &mut output)
                            {
                                output_len = length;
                            } else {
                                plan = None;
                                assigned.reset();
                                activation.reset();
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            plan = None;
                            assigned.reset();
                            activation.reset();
                        }
                    }
                }
            }
        }
        core::hint::spin_loop();
    }
}
