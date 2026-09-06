#![no_std]
#![no_main]

use conduit_avr_promicro_host::{boot::BootIdentity, provider::AvrCreateUart, usb_line::UsbLine};
use conduit_create_oi::{query_create1_group_zero, CreateOiFailure, PRESENTATION_START};
use panic_halt as _;

const REPORT_MAGIC: u8 = 0xa5;

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);
    let boot = BootIdentity::acquire(arduino_hal::Eeprom::new(peripherals.EEPROM));
    let _power_toggle = pins.d4.into_floating_input();
    let _charging = pins.d5.into_floating_input();
    let uart = arduino_hal::default_serial!(peripherals, pins, 57_600);
    let mut create = AvrCreateUart::new(uart);

    // Deliberately finite HIL diagnostic: every transmitted byte comes from
    // the shared motion-free Create 1 presentation program. It establishes
    // Full, sounds one song, then performs two bounded reads and stops.
    let report = probe(&mut create);
    let mut usb = UsbLine::new(peripherals.USB_DEVICE, peripherals.PLL, boot.usb_serial);
    let mut requested = false;
    let mut written = 0;
    loop {
        usb.poll();
        if !requested {
            let mut trigger = [0_u8; 1];
            if usb.read(&mut trigger).is_ok_and(|length| length == 1) {
                requested = true;
            }
        } else if written < report.len() {
            if let Ok(length) = usb.write(&report[written..]) {
                written += length;
            }
        }
        core::hint::spin_loop();
    }
}

fn probe<P: conduit_create_oi::CreateUartProvider>(provider: &mut P) -> [u8; 12] {
    if provider.write_all(&PRESENTATION_START).is_err() {
        return failure_report(1, CreateOiFailure::WriteFailed);
    }
    arduino_hal::delay_ms(250);
    let group = match query_create1_group_zero(provider, 10_000) {
        Ok(group) => group,
        Err(failure) => return failure_report(2, failure),
    };
    let voltage = group.millivolts.to_le_bytes();
    let current = group.milliamps.to_le_bytes();
    let charge = group.charge_mah.to_le_bytes();
    let capacity = group.capacity_mah.to_le_bytes();
    [
        REPORT_MAGIC,
        0,
        group.charging_state,
        voltage[0],
        voltage[1],
        current[0],
        current[1],
        group.temperature_celsius as u8,
        charge[0],
        charge[1],
        capacity[0],
        capacity[1],
    ]
}

const fn failure_report(stage: u8, failure: CreateOiFailure) -> [u8; 12] {
    [REPORT_MAGIC, stage, failure_code(failure), 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

const fn failure_code(failure: CreateOiFailure) -> u8 {
    match failure {
        CreateOiFailure::ProviderUnavailable => 10,
        CreateOiFailure::WrongUartProfile { .. } => 11,
        CreateOiFailure::WriteFailed => 12,
        CreateOiFailure::ReadFailed => 13,
        CreateOiFailure::Timeout => 14,
        CreateOiFailure::DeviceNoResponse => 15,
        CreateOiFailure::UnsupportedPacket(_) => 16,
        CreateOiFailure::TruncatedFrame => 17,
        CreateOiFailure::MalformedFrame => 18,
        CreateOiFailure::SynchronizationLimit { .. } => 19,
    }
}
