use super::*;
use std::{collections::BTreeMap, vec::Vec};
struct Io {
    registers: BTreeMap<usize, u32>,
    writes: Vec<(usize, u32)>,
    clock: u32,
    clock_step: u32,
    stuck_reset: bool,
    enable_port: bool,
    overcurrent_on_reset: bool,
}
impl Io {
    fn new(connected: bool) -> Self {
        Self {
            registers: BTreeMap::from([
                (CORE_ID, 0x4f54_280a),
                (RESET, AHB_IDLE),
                (INTERRUPT_STATUS, 1),
                (HARDWARE_2, (2 << 3) | (7 << 14)),
                (HARDWARE_3, 4096 << 16),
                (PORT, if connected { CONNECTED } else { 0 }),
            ]),
            writes: Vec::new(),
            clock: u32::MAX - 50_000,
            clock_step: 100,
            stuck_reset: false,
            enable_port: true,
            overcurrent_on_reset: false,
        }
    }
}
impl RegisterIo for Io {
    fn read(&mut self, offset: usize) -> u32 {
        *self.registers.get(&offset).unwrap_or(&0)
    }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
        match offset {
            RESET => {
                self.registers
                    .insert(RESET, AHB_IDLE | if self.stuck_reset { value } else { 0 });
            }
            INTERRUPT_STATUS => {
                self.registers.insert(offset, 1);
            } // Host role is read-only.
            PORT => {
                let old = self.read(PORT);
                let mut next = (old & (CONNECTED | ENABLED | OVERCURRENT))
                    | (value & (PORT_POWER | PORT_RESET));
                if old & PORT_RESET != 0 && value & PORT_RESET == 0 && self.enable_port {
                    next |= ENABLED;
                }
                if value & PORT_RESET != 0 && self.overcurrent_on_reset {
                    next |= OVERCURRENT;
                }
                self.registers.insert(offset, next);
            }
            _ => {
                self.registers.insert(offset, value);
            }
        }
    }
    fn now_us(&mut self) -> u32 {
        self.clock = self.clock.wrapping_add(self.clock_step);
        self.clock
    }
}
#[test]
fn preparation_is_class_neutral_bounded_and_wrap_safe() {
    assert_eq!(Refusal::Timeout.as_str(), "dwc2-deadline");
    let mut controller = Controller::new(Io::new(true));
    let facts = controller.prepare().unwrap();
    assert_eq!(facts.channels, 8);
    assert_eq!(facts.fifo_words, 4096);
    assert_eq!(
        facts.root_port & (CONNECTED | ENABLED | PORT_POWER),
        CONNECTED | ENABLED | PORT_POWER
    );
    assert!(controller.is_prepared());
    assert_eq!(controller.prepare(), Err(Refusal::AlreadyPrepared));
    assert_eq!(controller.io.read(AHB) & ((1 << 5) | 1), 0);
    assert_eq!(controller.io.read(INTERRUPT_MASK), 0);
    for (offset, value) in &controller.io.writes {
        if *offset == PORT {
            assert_eq!(value & PORT_WRITE_CLEAR, 0);
        }
    }
}
#[test]
fn absent_attachment_is_not_a_fabricated_device() {
    let mut controller = Controller::new(Io::new(false));
    let facts = controller.prepare().unwrap();
    assert_eq!(facts.root_port & CONNECTED, 0);
    assert_eq!(facts.root_port & PORT_RESET, 0);
    assert!(
        !controller
            .io
            .writes
            .iter()
            .any(|(offset, value)| *offset == PORT && value & PORT_RESET != 0)
    );
}
#[test]
fn foreign_core_gets_no_register_writes() {
    let mut io = Io::new(true);
    io.registers.insert(CORE_ID, 0xdead_beef);
    let mut controller = Controller::new(io);
    assert_eq!(controller.prepare(), Err(Refusal::UnsupportedCore));
    assert!(controller.io.writes.is_empty());
}
#[test]
fn reset_timeout_and_failed_enable_leave_power_and_reset_off() {
    for stuck in [true, false] {
        let mut io = Io::new(true);
        io.stuck_reset = stuck;
        io.enable_port = false;
        let mut controller = Controller::new(io);
        assert_eq!(
            controller.prepare(),
            Err(if stuck {
                Refusal::Timeout
            } else {
                Refusal::PortNotEnabled
            })
        );
        assert!(!controller.is_prepared());
        assert_eq!(controller.io.read(PORT) & (PORT_POWER | PORT_RESET), 0);
        assert_eq!(controller.io.read(INTERRUPT_MASK), 0);
    }
}
#[test]
fn overcurrent_during_port_reset_removes_power() {
    let mut io = Io::new(true);
    io.overcurrent_on_reset = true;
    let mut controller = Controller::new(io);
    assert_eq!(controller.prepare(), Err(Refusal::Overcurrent));
    assert_eq!(controller.io.read(PORT) & (PORT_POWER | PORT_RESET), 0);
}
#[test]
fn small_fifo_and_non_dma_hardware_are_refused_without_dma_submission() {
    for (offset, value) in [(HARDWARE_3, 512 << 16), (HARDWARE_2, 0)] {
        let mut io = Io::new(true);
        io.registers.insert(offset, value);
        let mut controller = Controller::new(io);
        assert_eq!(controller.prepare(), Err(Refusal::UnsupportedHardware));
        assert_eq!(controller.io.read(AHB) & (1 << 5), 0);
    }
}
#[test]
fn stopped_clock_cannot_make_a_wait_infinite() {
    let mut io = Io::new(true);
    io.clock_step = 0;
    io.stuck_reset = true;
    let mut controller = Controller::new(io);
    assert_eq!(controller.prepare(), Err(Refusal::WorkPressure));
}
