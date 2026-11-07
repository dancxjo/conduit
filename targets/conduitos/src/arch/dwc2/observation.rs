//! Non-destructive register facts at the exact packet retirement boundary.
use super::{Controller, PacketError, RegisterIo, packet::*, registers::*};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketFailure {
    pub phase: &'static str,
    pub outcome: Option<PacketError>,
    pub character: u32,
    pub status: u32,
    pub size: u32,
    pub split: u32,
    pub interrupts: u32,
    pub reset: u32,
    pub port: u32,
    pub tx_space: u32,
}
impl<I: RegisterIo> Controller<I> {
    pub(super) fn observe_failure(&mut self, phase: &'static str, outcome: Option<PacketError>) {
        let observation = PacketFailure {
            phase,
            outcome,
            character: self.io.read(CHARACTER),
            status: self.io.read(STATUS),
            size: self.io.read(0x510),
            split: self.io.read(0x504),
            interrupts: self.io.read(INTERRUPT_STATUS),
            reset: self.io.read(RESET),
            port: self.io.read(PORT),
            tx_space: self.io.read(TX_SPACE),
        };
        self.io.packet_failure(observation);
    }
}
