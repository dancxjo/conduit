#![no_std]
#![no_main]

use conduit_avr_promicro_host::{boot::BootIdentity, provider::AvrCreateUart, usb_line::UsbLine};
use conduit_create_oi::{
    encode_query_sensor, query_create1_group_zero, read_query_sensor_packet, write_command,
    CreateOiFailure, UartProfile, PRESENTATION_DEFINE_SONG, PRESENTATION_FULL,
    PRESENTATION_PLAY_SONG, PRESENTATION_START,
};
use panic_halt as _;

const MODE_PACKET: u8 = 35;
const REPORT_MAGIC: u8 = 0xa5;

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);
    let boot = BootIdentity::acquire(arduino_hal::Eeprom::new(peripherals.EEPROM));
    let _power_toggle = pins.d4.into_floating_input();
    let _charging = pins.d5.into_floating_input();
    let uart = arduino_hal::default_serial!(peripherals, pins, 19_200);
    let mut create = AvrCreateUart::with_profile(uart, UartProfile::CREATE_OI_19200);

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

fn probe<P: conduit_create_oi::CreateUartProvider>(provider: &mut P) -> [u8; 4] {
    for (stage, command) in [
        (1, &PRESENTATION_START[..]),
        (2, &PRESENTATION_FULL[..]),
        (3, &PRESENTATION_DEFINE_SONG[..]),
        (4, &PRESENTATION_PLAY_SONG[..]),
    ] {
        if provider.write_all(command).is_err() {
            let failure = CreateOiFailure::WriteFailed;
            return [REPORT_MAGIC, stage, failure_code(failure), 0];
        }
    }
    let query = encode_query_sensor(MODE_PACKET).unwrap();
    if let Err(failure) = write_command(provider, &query) {
        return [REPORT_MAGIC, 5, failure_code(failure), 0];
    }
    let mode = match read_query_sensor_packet(provider, MODE_PACKET, 10_000) {
        Ok(packet) => packet.bytes()[0],
        Err(failure) => return [REPORT_MAGIC, 6, failure_code(failure), 0],
    };
    let group = match query_create1_group_zero(provider, 10_000) {
        Ok(group) => group,
        Err(failure) => return [REPORT_MAGIC, 7, failure_code(failure), mode],
    };
    let contact = u8::from(group.left_bumper_pressed) << 1
        | u8::from(group.right_bumper_pressed) << 2;
    [REPORT_MAGIC, 0, mode, contact]
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
