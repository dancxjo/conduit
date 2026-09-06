#![no_std]
#![no_main]

use conduit_assigned_plan::{
    encode_assigned_execution_receipt, ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES,
};
use conduit_avr_promicro_host::{
    activation_receiver::ActivationReceiver,
    assigned_receiver::{AssignedReceiver, HOST_IDENTITY, MAX_ENCODED_BYTES},
    boot::BootIdentity,
    executor::execute_contact,
    provider,
    usb_line::UsbLine,
};
use panic_halt as _;

type ContactPlan = conduit_avr_promicro_host::assigned_receiver::ValidatedContactPlan;
const OUTPUT_BYTES: usize = ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES + 1;

struct RuntimeState {
    exchange: [u8; MAX_ENCODED_BYTES],
    assigned: AssignedReceiver,
    activation: ActivationReceiver,
    plan: Option<ContactPlan>,
    output_len: usize,
    output_offset: usize,
}

impl RuntimeState {
    const fn new() -> Self {
        Self {
            exchange: [0; MAX_ENCODED_BYTES],
            assigned: AssignedReceiver::new(),
            activation: ActivationReceiver::new(),
            plan: None,
            output_len: 0,
            output_offset: 0,
        }
    }

    fn reset_exchange(&mut self) {
        self.output_len = 0;
        self.output_offset = 0;
        self.plan = None;
        self.assigned.reset();
        self.activation.reset();
    }
}

static mut RUNTIME: RuntimeState = RuntimeState::new();

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
    // SAFETY: `main` is the sole execution context, interrupts are disabled,
    // and this unique reference lives for the remainder of the program.
    let runtime = unsafe { &mut *core::ptr::addr_of_mut!(RUNTIME) };

    loop {
        host_line.poll();
        if runtime.output_offset < runtime.output_len {
            let end = (runtime.output_offset + 32).min(runtime.output_len);
            if let Ok(written) =
                host_line.write(&runtime.exchange[runtime.output_offset..end])
            {
                runtime.output_offset += written;
                if runtime.output_offset == runtime.output_len {
                    runtime.reset_exchange();
                }
            }
            continue;
        }
        let mut incoming = [0_u8; 32];
        if let Ok(length) = host_line.read(&mut incoming) {
            if length != 0 {
                if runtime.plan.is_none() {
                    match runtime
                        .assigned
                        .push(&mut runtime.exchange, &incoming[..length])
                    {
                        Ok(Some(_)) => match runtime.assigned.validate(
                            &runtime.exchange,
                            host,
                            boot.assigned,
                        ) {
                            Ok(validated) => runtime.plan = Some(validated),
                            Err(_) => runtime.assigned.reset(),
                        },
                        Ok(None) => {}
                        Err(_) => runtime.assigned.reset(),
                    }
                } else {
                    match runtime
                        .activation
                        .push(&mut runtime.exchange, &incoming[..length])
                    {
                        Ok(Some(active)) => {
                            let mut value = [0_u8; 1];
                            let receipt = match runtime.plan {
                                Some(validated) => execute_contact(
                                    validated,
                                    active,
                                    &mut create,
                                    2_000,
                                    &mut value,
                                ),
                                None => {
                                    runtime.reset_exchange();
                                    continue;
                                }
                            };
                            if let Ok(length) = encode_assigned_execution_receipt(
                                receipt,
                                &mut runtime.exchange[..OUTPUT_BYTES],
                            )
                            {
                                runtime.output_len = length;
                            } else {
                                runtime.reset_exchange();
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            runtime.reset_exchange();
                        }
                    }
                }
            }
        }
        core::hint::spin_loop();
    }
}
