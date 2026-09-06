use arduino_hal::Eeprom;
use conduit_assigned_plan::AssignedIdentity;

const COUNTER_OFFSET: u16 = 0;
const COUNTER_BYTES: usize = 4;
const RECORD_BYTES: usize = COUNTER_BYTES * 2;

pub struct BootIdentity {
    pub assigned: AssignedIdentity,
    pub usb_serial: &'static str,
}

impl BootIdentity {
    pub fn acquire(mut eeprom: Eeprom) -> Self {
        let mut record = [0_u8; RECORD_BYTES];
        let _ = eeprom.read(COUNTER_OFFSET, &mut record);
        let valid = record[..COUNTER_BYTES]
            .iter()
            .zip(&record[COUNTER_BYTES..])
            .all(|(value, inverse)| *value == !*inverse);
        let previous = if valid {
            u32::from_le_bytes(record[..COUNTER_BYTES].try_into().unwrap())
        } else {
            u32::from_le_bytes(record[..COUNTER_BYTES].try_into().unwrap_or([0; 4]))
        };
        let counter = previous.wrapping_add(1);
        let value = counter.to_le_bytes();
        let inverse = value.map(|byte| !byte);
        let _ = eeprom.write(COUNTER_OFFSET, &value);
        let _ = eeprom.write(COUNTER_OFFSET + COUNTER_BYTES as u16, &inverse);

        let usb_serial = serial(counter);
        Self {
            assigned: AssignedIdentity::from_text(usb_serial),
            usb_serial,
        }
    }
}

fn serial(counter: u32) -> &'static str {
    static mut SERIAL: [u8; 12] = *b"avr-00000000";
    const HEX: &[u8; 16] = b"0123456789abcdef";
    // SAFETY: called once during boot before USB or interrupts are enabled;
    // the buffer then remains immutable for the lifetime of the firmware.
    unsafe {
        let pointer = core::ptr::addr_of_mut!(SERIAL).cast::<u8>();
        for index in 0..8 {
            let shift = (7 - index) * 4;
            pointer
                .add(4 + index)
                .write(HEX[((counter >> shift) & 0x0f) as usize]);
        }
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(pointer, 12))
    }
}
