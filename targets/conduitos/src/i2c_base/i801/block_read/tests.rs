use super::*;
use alloc::{collections::VecDeque, vec, vec::Vec};

struct Registers {
    statuses: VecDeque<u8>,
    fallback: u8,
    octets: VecDeque<u8>,
    writes: Vec<(u16, u8)>,
    stop_works: bool,
}
impl I801Registers for Registers {
    fn read(&mut self, offset: u16) -> u8 {
        match offset {
            STATUS => self.statuses.pop_front().unwrap_or(self.fallback),
            BLOCK_DATA => self.octets.pop_front().expect("one admitted byte read"),
            _ => panic!("unexpected register read"),
        }
    }
    fn write(&mut self, offset: u16, value: u8) {
        self.writes.push((offset, value));
        if offset == CONTROL && value == KILL {
            self.statuses.clear();
            self.fallback = if self.stop_works { FAILED } else { BUSY };
        }
    }
}
fn registers(count: usize, terminal: u8) -> Registers {
    let mut statuses = VecDeque::from([0]);
    statuses.extend(core::iter::repeat_n(BUSY | BYTE_DONE | IN_USE, count));
    statuses.push_back(terminal | IN_USE);
    Registers {
        statuses,
        fallback: terminal | IN_USE,
        octets: (0..count).map(|index| index as u8 ^ 0xa5).collect(),
        writes: vec![],
        stop_works: true,
    }
}

#[test]
fn admitted_block_reads_preserve_prefix_and_exact_extent() {
    for length in 2..=32 {
        for spd_write_disabled in [false, true] {
            let mut controller = unsafe {
                I801Controller::new(registers(length, COMPLETE), length as u32 + 1)
                    .unwrap()
                    .with_i2c_block_reads(spd_write_disabled)
            };
            let request = I2cTransaction::new(0x76, &[0x88], length as u8).unwrap();
            let mut input = [0; 32];
            assert_eq!(
                controller.transact(&request, &mut input[..length]),
                Ok(length)
            );
            for (index, byte) in input[..length].iter().enumerate() {
                assert_eq!(*byte, index as u8 ^ 0xa5);
            }
            assert_eq!(input[length..], [0; 32][length..]);
            let writes = &controller.registers.writes;
            assert_eq!(writes[1], (ADDRESS, 0xec | u8::from(spd_write_disabled)));
            assert_eq!(writes[2], (DATA1, 0x88));
            assert_eq!(writes[3], (CONTROL, I2C_BLOCK | START));
            assert_eq!(writes[4 + length - 2], (CONTROL, I2C_BLOCK | LAST_BYTE));
            assert_eq!(
                writes
                    .iter()
                    .filter(|entry| **entry == (STATUS, BYTE_DONE))
                    .count(),
                length
            );
            assert_eq!(writes.last(), Some(&(STATUS, IN_USE)));
        }
    }
}

#[test]
fn early_completion_is_short_without_another_transaction() {
    let mut controller = unsafe {
        I801Controller::new(registers(3, COMPLETE), 8)
            .unwrap()
            .with_i2c_block_reads(false)
    };
    let mut input = [0; 8];
    assert_eq!(
        controller.transact(&I2cTransaction::new(0x76, &[0xf7], 8).unwrap(), &mut input),
        Ok(3)
    );
    assert_eq!(input[3..], [0; 5]);
    assert_eq!(
        controller
            .registers
            .writes
            .iter()
            .filter(|(offset, value)| *offset == CONTROL && value & START != 0)
            .count(),
        1
    );
}

#[test]
fn shared_poll_budget_and_failed_stop_preserve_provider_loss() {
    for stop_works in [false, true] {
        let mut regs = registers(2, BUSY);
        regs.stop_works = stop_works;
        let mut controller = unsafe {
            I801Controller::new(regs, 3)
                .unwrap()
                .with_i2c_block_reads(false)
        };
        let request = I2cTransaction::new(0x76, &[0x88], 24).unwrap();
        let mut input = [0; 24];
        assert_eq!(
            controller.transact(&request, &mut input),
            Err(if stop_works {
                I2cDisposition::TimedOut
            } else {
                I2cDisposition::ProviderLost
            })
        );
        if !stop_works {
            let writes = controller.registers.writes.len();
            assert_eq!(
                controller.transact(&request, &mut input),
                Err(I2cDisposition::ProviderLost)
            );
            assert_eq!(controller.registers.writes.len(), writes);
            assert_ne!(controller.registers.writes.last(), Some(&(STATUS, IN_USE)));
        }
    }
}

#[test]
fn loss_errors_and_foreign_ownership_do_not_consume_bytes() {
    for (status, expected) in [
        (DEVICE_ERROR, I2cDisposition::DeviceError),
        (ARBITRATION, I2cDisposition::ArbitrationLost),
        (FAILED, I2cDisposition::ProviderLost),
        (FAILED | DEVICE_ERROR, I2cDisposition::ProviderLost),
    ] {
        for busy in [0, BUSY] {
            let mut controller = unsafe {
                I801Controller::new(registers(0, status | busy), 4)
                    .unwrap()
                    .with_i2c_block_reads(true)
            };
            assert_eq!(
                controller.transact(
                    &I2cTransaction::new(0x76, &[0x88], 24).unwrap(),
                    &mut [0; 24]
                ),
                Err(expected)
            );
        }
    }
    for initial in [IN_USE, BUSY] {
        let mut regs = registers(24, COMPLETE);
        regs.statuses[0] = initial;
        let mut controller = unsafe {
            I801Controller::new(regs, 32)
                .unwrap()
                .with_i2c_block_reads(false)
        };
        assert_eq!(
            controller.transact(
                &I2cTransaction::new(0x76, &[0x88], 24).unwrap(),
                &mut [0; 24]
            ),
            Err(I2cDisposition::Refused)
        );
        assert_eq!(
            controller.registers.writes,
            if initial == IN_USE {
                vec![]
            } else {
                vec![(STATUS, IN_USE)]
            }
        );
    }
}

#[test]
fn wrong_output_extent_refuses_before_register_access() {
    let mut controller = unsafe {
        I801Controller::new(registers(24, COMPLETE), 32)
            .unwrap()
            .with_i2c_block_reads(false)
    };
    assert_eq!(
        controller.transact(
            &I2cTransaction::new(0x76, &[0x88], 24).unwrap(),
            &mut [0; 23],
        ),
        Err(I2cDisposition::Refused)
    );
    assert_eq!(controller.registers.statuses.front(), Some(&0));
    assert!(controller.registers.writes.is_empty());
}

#[test]
fn extra_byte_quarantines_without_reading_beyond_admission() {
    let mut controller = unsafe {
        I801Controller::new(registers(3, COMPLETE), 4)
            .unwrap()
            .with_i2c_block_reads(false)
    };
    let request = I2cTransaction::new(0x76, &[0x88], 2).unwrap();
    assert_eq!(
        controller.transact(&request, &mut [0; 2]),
        Err(I2cDisposition::ProviderLost)
    );
    assert_eq!(controller.registers.octets.len(), 1);
    let writes = controller.registers.writes.len();
    assert_eq!(
        controller.transact(&request, &mut [0; 2]),
        Err(I2cDisposition::ProviderLost)
    );
    assert_eq!(controller.registers.writes.len(), writes);
}
