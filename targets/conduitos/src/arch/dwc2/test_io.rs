use super::packet::*;
use super::*;
use std::{collections::VecDeque, vec::Vec};
pub(super) struct Io {
    pub(super) writes: Vec<(usize, u32)>,
    pub(super) rx: VecDeque<u32>,
    pub(super) data: VecDeque<u32>,
    pub(super) clock: u32,
    pub(super) running: bool,
    pub(super) finish: u32,
    pub(super) stuck_halt: bool,
    pub(super) port: u32,
    pub(super) residue: u32,
    pub(super) script: VecDeque<(u32, Vec<u32>, Vec<u32>)>,
}
impl Io {
    pub(super) fn new() -> Self {
        Self {
            writes: Vec::new(),
            rx: VecDeque::new(),
            data: VecDeque::new(),
            clock: u32::MAX - 5000,
            running: false,
            finish: HALTED | 1,
            stuck_halt: false,
            residue: 0,
            script: VecDeque::new(),
            port: CONNECTED | ENABLED | PORT_POWER,
        }
    }
}
impl RegisterIo for Io {
    fn read(&mut self, offset: usize) -> u32 {
        match offset {
            PORT => self.port,
            0x510 => self.residue,
            TX_SPACE => (1 << 16) | 256,
            INTERRUPT_STATUS => {
                if !self.rx.is_empty() {
                    1 << 4
                } else {
                    0
                }
            }
            RX_POP => self.rx.pop_front().unwrap(),
            FIFO => self.data.pop_front().unwrap(),
            STATUS => {
                if self.finish & HALTED != 0 {
                    self.running = false;
                }
                self.finish
            }
            CHARACTER => {
                if self.running {
                    ENABLE
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
        if offset == CHARACTER {
            if value & HALT != 0 {
                if !self.stuck_halt {
                    self.running = false;
                    self.finish = HALTED;
                }
            } else {
                self.running = value & ENABLE != 0;
                if let Some((finish, rx, data)) = self.script.pop_front() {
                    self.finish = finish;
                    self.rx = rx.into();
                    self.data = data.into();
                }
            }
        }
    }
    fn now_us(&mut self) -> u32 {
        self.clock = self.clock.wrapping_add(1000);
        self.clock
    }
}
