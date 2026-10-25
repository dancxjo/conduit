use super::*;
use std::collections::VecDeque;

struct Mock {
    configuration: [u32; 5],
    flags: VecDeque<u32>,
    reads: usize,
    writes: Vec<(usize, u32)>,
}
impl Mock {
    fn new(flags: &[u32]) -> Self {
        Self {
            configuration: [0x301, 0x70, 26, 3, 0],
            flags: flags.iter().copied().collect(),
            reads: 0,
            writes: Vec::new(),
        }
    }
}
impl Registers for Mock {
    fn read(&mut self, offset: usize) -> u32 {
        match offset {
            CR => self.configuration[0],
            LCRH => self.configuration[1],
            IBRD => self.configuration[2],
            FBRD => self.configuration[3],
            IMSC => self.configuration[4],
            FR => {
                self.reads += 1;
                self.flags.pop_front().expect("unexpected status read")
            }
            _ => panic!("unexpected register read"),
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
    }
}
const DIVISOR: u32 = 26 * 64 + 3;
#[test]
fn exact_clock_divisors_and_invalid_inputs() {
    assert_eq!(expected_divisor(48_000_000, 115200), Ok(DIVISOR));
    assert_eq!(expected_divisor(3_000_000, 115200), Ok(104));
    for (clock, baud) in [(0, 1), (1, 0), (1, 1), (u32::MAX, 1)] {
        assert_eq!(
            expected_divisor(clock, baud),
            Err(SerialRefusal::InvalidClockOrBaud)
        );
    }
}
#[test]
fn validates_mapping_without_physical_pointer_assumptions() {
    for base in [0, 1, usize::MAX - 3] {
        assert_eq!(validate_mapping(base), Err(SerialRefusal::InvalidMapping));
    }
    assert_eq!(validate_mapping(PHYSICAL_BASE), Ok(()));
}
#[test]
fn writes_exact_bytes_without_touching_control_or_newline_translation() {
    let mut m = Mock::new(&[TXFF, 0, 0, 0]);
    assert_eq!(transmit(&mut m, DIVISOR, b"a\n\0", 4), Ok(3));
    assert_eq!(m.writes, [(DR, 97), (DR, 10), (DR, 0)]);
    assert_eq!(m.reads, 4);
}
#[test]
fn total_budget_is_shared_across_bytes_and_reports_irreversible_prefix() {
    let mut m = Mock::new(&[0, TXFF, TXFF]);
    assert_eq!(
        transmit(&mut m, DIVISOR, b"ab", 3),
        Err(TransmitRefusal {
            accepted_bytes: 1,
            reason: SerialRefusal::PollBudgetExhausted
        })
    );
    assert_eq!(m.reads, 3);
    assert_eq!(m.writes, [(DR, 97)]);
}
#[test]
fn invalid_work_bounds_do_not_access_registers() {
    let mut m = Mock::new(&[]);
    for budget in [0, MAXIMUM_STATUS_POLLS + 1] {
        assert_eq!(
            transmit(&mut m, DIVISOR, b"a", budget).unwrap_err().reason,
            SerialRefusal::InvalidBudget
        );
        assert_eq!(
            flush(&mut m, DIVISOR, budget),
            Err(SerialRefusal::InvalidBudget)
        );
    }
    assert_eq!(
        transmit(&mut m, DIVISOR, &[0; MAXIMUM_WRITE_BYTES + 1], 1)
            .unwrap_err()
            .reason,
        SerialRefusal::MessageTooLong
    );
    assert!(m.writes.is_empty());
}
#[test]
fn rejects_disabled_flow_control_loopback_format_divisor_and_interrupts() {
    for (index, value) in [
        (0, 0),
        (0, 0x8101),
        (0, 0x181),
        (1, 0x60),
        (1, 0x72),
        (2, 25),
        (3, 2),
        (4, 1),
    ] {
        let mut m = Mock::new(&[]);
        m.configuration[index] = value;
        assert_eq!(
            transmit(&mut m, DIVISOR, b"a", 1).unwrap_err().reason,
            SerialRefusal::ConfigurationMismatch
        );
        assert!(m.writes.is_empty());
    }
}
#[test]
fn unavailable_control_and_flags_never_write_data() {
    let mut m = Mock::new(&[]);
    m.configuration[0] = u32::MAX;
    assert_eq!(
        check_configuration(&mut m, DIVISOR),
        Err(SerialRefusal::Unavailable)
    );
    let mut m = Mock::new(&[u32::MAX]);
    assert_eq!(
        transmit(&mut m, DIVISOR, b"a", 1).unwrap_err().reason,
        SerialRefusal::Unavailable
    );
    assert!(m.writes.is_empty());
}
#[test]
fn flush_requires_empty_and_not_busy_and_has_no_writes() {
    let mut m = Mock::new(&[0, TXFE | BUSY, TXFE]);
    assert_eq!(flush(&mut m, DIVISOR, 3), Ok(()));
    assert_eq!(m.reads, 3);
    assert!(m.writes.is_empty());
    let mut m = Mock::new(&[TXFE | BUSY]);
    assert_eq!(
        flush(&mut m, DIVISOR, 1),
        Err(SerialRefusal::PollBudgetExhausted)
    );
}
#[test]
fn rechecks_configuration_each_call_without_altering_it() {
    let mut m = Mock::new(&[0]);
    assert_eq!(transmit(&mut m, DIVISOR, b"a", 1), Ok(1));
    m.configuration[0] = 0;
    assert_eq!(
        flush(&mut m, DIVISOR, 1),
        Err(SerialRefusal::ConfigurationMismatch)
    );
    assert_eq!(m.writes, [(DR, 97)]);
}

#[test]
fn preserves_allowed_receive_rts_and_reserved_read_bits() {
    let mut m = Mock::new(&[0]);
    m.configuration[0] = 0x301 | 0x800 | 0x3000 | 0x78;
    assert_eq!(transmit(&mut m, DIVISOR, b"a", 1), Ok(1));
    assert_eq!(m.writes, [(DR, 97)]);
}
