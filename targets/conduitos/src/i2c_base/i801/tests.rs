use super::*;
use alloc::{vec, vec::Vec};

struct Registers {
    status: u8,
    started: bool,
    after_start: u8,
    stop_works: bool,
    reads: u32,
    writes: Vec<(u16, u8)>,
}
impl I801Registers for Registers {
    fn read(&mut self, offset: u16) -> u8 {
        self.reads += 1;
        match offset {
            STATUS => {
                let status = self.status;
                self.status |= IN_USE;
                status
            }
            DATA => 0x91,
            _ => 0,
        }
    }
    fn write(&mut self, offset: u16, value: u8) {
        self.writes.push((offset, value));
        if offset == CONTROL && value & START != 0 {
            self.started = true;
            self.status = self.after_start | IN_USE;
        } else if offset == CONTROL && value == KILL && self.stop_works {
            self.status = FAILED | IN_USE;
        } else if offset == STATUS {
            self.status &= !value;
        }
    }
}
fn registers(after_start: u8, stop_works: bool) -> Registers {
    Registers {
        status: 0,
        started: false,
        after_start,
        stop_works,
        reads: 0,
        writes: vec![],
    }
}

#[test]
fn byte_data_has_exact_bus_geometry_without_register_meaning() {
    for prefix in 0..=255 {
        let mut controller = I801Controller::new(registers(COMPLETE, true), 4).unwrap();
        let bytes = [prefix];
        let request = I2cTransaction::new(0x76, &bytes, 1).unwrap();
        let mut input = [0];
        assert_eq!(controller.transact(&request, &mut input), Ok(1));
        assert_eq!(input, [0x91]);
        assert_eq!(
            controller.registers.writes,
            [
                (STATUS, CLEAR_STATUS),
                (ADDRESS, 0xed),
                (COMMAND, prefix),
                (CONTROL, BYTE_DATA | START),
                (STATUS, CLEAR_STATUS),
                (STATUS, IN_USE)
            ]
        );
    }
    let mut controller = I801Controller::new(registers(COMPLETE, true), 4).unwrap();
    let request = I2cTransaction::new(0x77, &[0xff, 0xab], 0).unwrap();
    assert_eq!(controller.transact(&request, &mut []), Ok(0));
    assert!(controller.registers.writes.contains(&(DATA, 0xab)));
}

#[test]
fn unsupported_or_busy_geometry_has_no_effect() {
    let mut controller = I801Controller::new(registers(COMPLETE, true), 4).unwrap();
    for (write, read) in [(&[1, 2, 3][..], 0), (&[1][..], 2), (&[][..], 32)] {
        let request = I2cTransaction::new(0x76, write, read).unwrap();
        assert_eq!(
            controller.transact(&request, &mut [0; 32][..usize::from(read)]),
            Err(I2cDisposition::Unsupported)
        );
        assert!(controller.registers.writes.is_empty());
    }
    controller.registers.status = BUSY;
    assert_eq!(
        controller.transact(&I2cTransaction::new(0x76, &[1], 0).unwrap(), &mut []),
        Err(I2cDisposition::Refused)
    );
    assert_eq!(controller.registers.writes, [(STATUS, IN_USE)]);
}

#[test]
fn bounded_timeout_and_failed_stop_are_different() {
    for stop_works in [false, true] {
        let mut controller = I801Controller::new(registers(BUSY, stop_works), 4).unwrap();
        let request = I2cTransaction::new(0x76, &[1], 0).unwrap();
        assert_eq!(
            controller.transact(&request, &mut []),
            Err(if stop_works {
                I2cDisposition::TimedOut
            } else {
                I2cDisposition::ProviderLost
            })
        );
        assert!(controller.registers.reads <= 9);
        if !stop_works {
            let writes = controller.registers.writes.len();
            assert_eq!(
                controller.transact(&request, &mut []),
                Err(I2cDisposition::ProviderLost)
            );
            assert_eq!(controller.registers.writes.len(), writes);
        }
    }
}

#[test]
fn bus_errors_and_revocation_cannot_emit_input() {
    for (status, expected) in [
        (DEVICE_ERROR, I2cDisposition::DeviceError),
        (ARBITRATION, I2cDisposition::ArbitrationLost),
        (FAILED, I2cDisposition::ProviderLost),
    ] {
        let mut controller = I801Controller::new(registers(status, true), 4).unwrap();
        let request = I2cTransaction::new(0x76, &[1], 1).unwrap();
        let mut input = [0];
        assert_eq!(controller.transact(&request, &mut input), Err(expected));
        assert_eq!(input, [0]);
        controller.revoke();
        let writes = controller.registers.writes.len();
        assert_eq!(
            controller.transact(&request, &mut input),
            Err(I2cDisposition::ProviderLost)
        );
        assert_eq!(controller.registers.writes.len(), writes);
    }
}

#[test]
fn another_native_semaphore_owner_is_preserved() {
    let mut registers = registers(COMPLETE, true);
    registers.status = IN_USE;
    let mut controller = I801Controller::new(registers, 4).unwrap();
    let request = I2cTransaction::new(0x76, &[1], 0).unwrap();
    assert_eq!(
        controller.transact(&request, &mut []),
        Err(I2cDisposition::Refused)
    );
    assert!(controller.registers.writes.is_empty());
    assert_eq!(controller.registers.status, IN_USE);
}
