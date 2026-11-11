use super::packet::*;
use super::*;
use std::{collections::VecDeque, vec::Vec};
pub(crate) struct ControlObservation {
    pub route: EndpointRoute,
    pub setup: [u8; 8],
    pub outcome: Result<usize, PacketError>,
    pub input: Vec<u8>,
}
pub(crate) struct Io {
    pub(crate) writes: Vec<(usize, u32)>,
    pub(crate) controls: Vec<ControlObservation>,
    pub(crate) rx: VecDeque<u32>,
    pub(crate) data: VecDeque<u32>,
    pub(crate) clock: u32,
    pub(crate) clock_step: u32,
    pub(crate) frame: u16,
    pub(crate) frame_step: u16,
    pub(crate) running: bool,
    pub(crate) finish: u32,
    pub(crate) stuck_halt: bool,
    pub(crate) halt_via_rx: bool,
    pub(crate) queue_slots: VecDeque<u8>,
    pub(crate) late_halt_data: bool,
    pub(crate) failures: Vec<PacketFailure>,
    pub(crate) port: u32,
    pub(crate) residue: u32,
    pub(crate) script: VecDeque<(u32, Vec<u32>, Vec<u32>)>,
}
impl Io {
    pub(crate) fn new() -> Self {
        Self {
            writes: Vec::new(),
            controls: Vec::new(),
            rx: VecDeque::new(),
            data: VecDeque::new(),
            clock: u32::MAX - 5000,
            clock_step: 1000,
            frame: 0,
            frame_step: 1,
            running: false,
            finish: HALTED | 1,
            stuck_halt: false,
            halt_via_rx: true,
            queue_slots: VecDeque::new(),
            late_halt_data: false,
            failures: Vec::new(),
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
            0x408 => {
                self.frame = self.frame.wrapping_add(self.frame_step);
                u32::from(self.frame)
            }
            0x410 | TX_SPACE => (u32::from(self.queue_slots.pop_front().unwrap_or(1)) << 16) | 256,
            INTERRUPT_STATUS => {
                if !self.rx.is_empty() {
                    1 << 4
                } else {
                    0
                }
            }
            RX_POP => {
                let status = self.rx.pop_front().unwrap();
                if (status >> 17) & 15 == 7 && status & 15 == 0 {
                    self.running = false;
                    self.finish |= HALTED;
                }
                status
            }
            FIFO => self.data.pop_front().unwrap(),
            STATUS => {
                if self.finish & HALTED != 0 {
                    self.running = false;
                }
                self.finish
            }
            CHARACTER if self.running => ENABLE,
            _ => 0,
        }
    }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
        if offset == CHARACTER {
            if value & HALT != 0 {
                if !self.stuck_halt && value & ENABLE != 0 {
                    if self.late_halt_data {
                        self.rx.push_back((2 << 17) | (4 << 4));
                        self.data.push_back(0xdeadbeef);
                    }
                    if self.halt_via_rx {
                        self.rx.push_back(7 << 17);
                    } else {
                        self.running = false;
                        self.finish |= HALTED;
                    }
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
    fn control_observation(
        &mut self,
        route: EndpointRoute,
        setup: [u8; 8],
        outcome: Result<usize, PacketError>,
        input: &[u8],
    ) {
        self.controls.push(ControlObservation {
            route,
            setup,
            outcome,
            input: input.to_vec(),
        });
    }
    fn packet_failure(&mut self, failure: PacketFailure) {
        self.failures.push(failure);
    }
    fn now_us(&mut self) -> u32 {
        self.clock = self.clock.wrapping_add(self.clock_step);
        self.clock
    }
}

#[allow(dead_code)] // Shared by native-owner tests, absent from the protocol-only facade.
pub(crate) fn prepared() -> Controller<Io> {
    Controller {
        io: Io::new(),
        prepared: true,
        quiescent: true,
    }
}

#[allow(dead_code)] // Native-owner tests share this model with controller tests.
pub(crate) fn registers(controller: &mut Controller<Io>) -> &mut Io {
    &mut controller.io
}
