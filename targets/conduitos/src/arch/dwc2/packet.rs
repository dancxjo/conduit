//! One polling/PIO packet on the directly attached high-speed device.
//! No descriptors, requests, address allocation, retries or class policy.
use super::{Controller, POLL_LIMIT, RegisterIo, registers::*};
pub(super) const CHARACTER: usize = 0x500;
const SPLIT: usize = 0x504;
pub(super) const STATUS: usize = 0x508;
const SIZE: usize = 0x510;
pub(super) const FIFO: usize = 0x1000;
pub(super) const RX_POP: usize = 0x20;
pub(super) const TX_SPACE: usize = 0x2c;
pub(super) const ENABLE: u32 = 1 << 31;
pub(super) const HALT: u32 = 1 << 30;
pub(super) const HALTED: u32 = 1 << 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketPid {
    Data0,
    Data1,
    Setup,
}
impl PacketPid {
    fn bits(self) -> u32 {
        match self {
            Self::Data0 => 0,
            Self::Data1 => 2,
            Self::Setup => 3,
        }
    }
}
/// Root-selected routing geometry, never constructed from a serialized claim.
/// Split transactions and periodic scheduling are deliberately separate work.
#[derive(Clone, Copy, Debug)]
pub struct RootPacket {
    pub address: u8,
    pub maximum_packet: u8,
    pub pid: PacketPid,
    pub input: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketError {
    NotPrepared,
    Geometry,
    UnsupportedSpeed,
    ProviderLost,
    Overcurrent,
    Nak,
    Nyet,
    Stall,
    Transaction,
    ReceiveEnvelope,
    Deadline,
    WorkPressure,
    Retirement,
}
impl<I: RegisterIo> Controller<I> {
    /// Synchronously transfers one EP0 packet, at most 64 bytes. Channel zero
    /// is held exclusively through acknowledged halt. No DMA or allocation.
    /// A refusal never silently resubmits a packet or changes its DATA PID.
    pub fn root_packet(
        &mut self,
        packet: RootPacket,
        bytes: &mut [u8],
    ) -> Result<usize, PacketError> {
        if !self.prepared {
            return Err(PacketError::NotPrepared);
        }
        if packet.address > 127
            || packet.maximum_packet == 0
            || packet.maximum_packet > 64
            || bytes.len() > usize::from(packet.maximum_packet)
            || (packet.input && bytes.len() != usize::from(packet.maximum_packet))
            || (packet.pid == PacketPid::Setup && (packet.input || bytes.len() != 8))
        {
            return Err(PacketError::Geometry);
        }
        let port = self.io.read(PORT);
        if port & OVERCURRENT != 0 {
            self.io.write(PORT, port_write(port) & !PORT_POWER);
            self.prepared = false;
            return Err(PacketError::Overcurrent);
        }
        if port & (CONNECTED | ENABLED | PORT_POWER) != CONNECTED | ENABLED | PORT_POWER {
            return Err(PacketError::ProviderLost);
        }
        if port & (3 << 17) != 0 {
            return Err(PacketError::UnsupportedSpeed);
        }
        // A previous unacknowledged hardware operation is never overwritten.
        if self.io.read(CHARACTER) & ENABLE != 0 {
            self.quiescent = false;
            self.prepared = false;
            return Err(PacketError::Retirement);
        }
        if !packet.input {
            let space = self.io.read(TX_SPACE);
            if space & 0xffff < bytes.len().div_ceil(4) as u32 || (space >> 16) & 0xff == 0 {
                return Err(PacketError::WorkPressure);
            }
        }
        self.io.write(STATUS, 0x7ff);
        self.io.write(SPLIT, 0);
        self.io.write(
            SIZE,
            (packet.pid.bits() << 29) | (1 << 19) | bytes.len() as u32,
        );
        let characteristic = (1 << 20)
            | u32::from(packet.maximum_packet)
            | (u32::from(packet.input) << 15)
            | (u32::from(packet.address) << 22);
        self.quiescent = false;
        self.io.write(CHARACTER, characteristic | ENABLE);
        if !packet.input {
            for chunk in bytes.chunks(4) {
                let mut word = [0; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                self.io.write(FIFO, u32::from_le_bytes(word));
            }
        }
        let result = self.poll_packet(packet.input, bytes);
        // Even timeout, removal and malformed FIFO observations must retire the
        // hardware channel before another caller can reuse it. Otherwise fence
        // this controller until a fresh machine-level reset/ownership epoch.
        let halted = self.io.read(STATUS) & HALTED != 0;
        let active = self.io.read(CHARACTER) & ENABLE != 0;
        if !halted && !active {
            self.prepared = false;
            return Err(PacketError::Retirement);
        }
        if active {
            self.io.write(CHARACTER, characteristic | ENABLE | HALT);
            if self.wait(STATUS, HALTED, HALTED, 100_000).is_err()
                || self.io.read(CHARACTER) & ENABLE != 0
            {
                self.prepared = false;
                return Err(PacketError::Retirement);
            }
        }
        self.quiescent = true;
        if result.is_err() {
            // This sole channel owns both FIFOs; discard its queued residue.
            self.io.write(RESET, (1 << 5) | (0x10 << 6));
            let tx = self.wait(RESET, 1 << 5, 0, 100_000);
            self.io.write(RESET, 1 << 4);
            let rx = self.wait(RESET, 1 << 4, 0, 100_000);
            if tx.is_err() || rx.is_err() {
                self.prepared = false;
                return Err(PacketError::Retirement);
            }
        }
        self.io.write(STATUS, 0x7ff);
        result
    }
    fn poll_packet(&mut self, input: bool, bytes: &mut [u8]) -> Result<usize, PacketError> {
        let start = self.io.now_us();
        let mut received = None;
        for _ in 0..POLL_LIMIT {
            if self.io.now_us().wrapping_sub(start) >= 100_000 {
                return Err(PacketError::Deadline);
            }
            let port = self.io.read(PORT);
            if port & OVERCURRENT != 0 {
                self.io.write(PORT, port_write(port) & !PORT_POWER);
                self.prepared = false;
                return Err(PacketError::Overcurrent);
            }
            if port & (CONNECTED | ENABLED | PORT_POWER) != CONNECTED | ENABLED | PORT_POWER {
                return Err(PacketError::ProviderLost);
            }
            if self.io.read(INTERRUPT_STATUS) & (1 << 4) != 0 {
                let status = self.io.read(RX_POP);
                let count = ((status >> 4) & 0x7ff) as usize;
                let data = (status >> 17) & 15 == 2;
                let valid =
                    input && data && status & 15 == 0 && received.is_none() && count <= bytes.len();
                for offset in (0..count).step_by(4) {
                    let word = self.io.read(FIFO).to_le_bytes();
                    if valid {
                        let take = (count - offset).min(4);
                        bytes[offset..offset + take].copy_from_slice(&word[..take]);
                    }
                }
                if data || count != 0 {
                    if !valid {
                        return Err(PacketError::ReceiveEnvelope);
                    }
                    received = Some(count);
                }
                // Drain RX observations before interpreting channel completion.
                continue;
            }
            let status = self.io.read(STATUS);
            if status & (1 << 3) != 0 {
                return Err(PacketError::Stall);
            }
            if status & (1 << 4) != 0 {
                return Err(PacketError::Nak);
            }
            if status & (1 << 6) != 0 {
                return Err(PacketError::Nyet);
            }
            if status & 0x784 != 0 {
                return Err(PacketError::Transaction);
            }
            if status & HALTED != 0 {
                if status & 1 == 0 || (!input && self.io.read(SIZE) & 0x7ffff != 0) {
                    return Err(PacketError::Transaction);
                }
                return if input {
                    received.ok_or(PacketError::ReceiveEnvelope)
                } else {
                    Ok(bytes.len())
                };
            }
        }
        Err(PacketError::WorkPressure)
    }
}
#[cfg(test)]
mod tests;
