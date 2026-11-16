//! One polling/PIO packet on the directly attached high-speed device.
//! No descriptors, requests, address allocation, retries or class policy.
use super::{Controller, EndpointRoute, POLL_LIMIT, RegisterIo, TransferType, registers::*};
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
#[cfg(test)]
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
#[derive(Clone, Copy)]
pub(super) struct WirePacket {
    pub route: EndpointRoute,
    pub pid: PacketPid,
    pub input: bool,
    pub split: u32,
    pub start_split: bool,
}
impl<I: RegisterIo> Controller<I> {
    /// Synchronously transfers one EP0 packet, at most 64 bytes. Channel zero
    /// is held exclusively through acknowledged halt. No DMA or allocation.
    /// A refusal never silently resubmits a packet or changes its DATA PID.
    #[cfg(test)]
    pub fn root_packet(
        &mut self,
        packet: RootPacket,
        bytes: &mut [u8],
    ) -> Result<usize, PacketError> {
        self.packet_on(
            EndpointRoute::control(packet.address, packet.maximum_packet),
            packet.pid,
            packet.input,
            bytes,
        )
    }
    pub(super) fn wire_packet(
        &mut self,
        packet: WirePacket,
        bytes: &mut [u8],
    ) -> Result<usize, PacketError> {
        if !self.prepared {
            return Err(PacketError::NotPrepared);
        }
        if packet.route.address > 127
            || packet.route.maximum_packet == 0
            || packet.route.maximum_packet > 64
            || bytes.len() > usize::from(packet.route.maximum_packet)
            || (packet.input && bytes.len() != usize::from(packet.route.maximum_packet))
            || (packet.pid == PacketPid::Setup
                && (packet.input
                    || packet.route.transfer_type != TransferType::Control
                    || (bytes.len() != 8 && !(packet.split & (1 << 16) != 0 && bytes.is_empty()))))
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
            self.observe_failure("already-active", None);
            self.quiescent = false;
            self.prepared = false;
            return Err(PacketError::Retirement);
        }
        let periodic = packet.route.transfer_type == TransferType::Interrupt;
        let space = self.io.read(if periodic { 0x410 } else { TX_SPACE });
        if (space >> 16) & 0xff == 0
            || (!packet.input && space & 0xffff < bytes.len().div_ceil(4) as u32)
        {
            return Err(PacketError::WorkPressure);
        }
        self.io.write(STATUS, 0x7ff);
        self.io.write(SPLIT, packet.split);
        self.io.write(
            SIZE,
            (packet.pid.bits() << 29) | (1 << 19) | bytes.len() as u32,
        );
        let characteristic = (1 << 20)
            | u32::from(packet.route.maximum_packet)
            | (u32::from(packet.input) << 15)
            | (u32::from(packet.route.address) << 22)
            | (u32::from(packet.route.endpoint) << 11)
            | (packet.route.transfer_type.bits() << 18)
            | (u32::from(packet.route.translator.is_some_and(|tt| tt.low_speed)) << 17)
            | if periodic {
                ((self.io.read(0x408) + 1) & 1) << 29
            } else {
                0
            };
        self.quiescent = false;
        self.io.write(CHARACTER, characteristic | ENABLE);
        if !packet.input {
            for chunk in bytes.chunks(4) {
                let mut word = [0; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                self.io.write(FIFO, u32::from_le_bytes(word));
            }
        }
        let mut result = self.poll_packet(
            packet.input && !packet.start_split,
            packet.start_split,
            bytes,
        );
        if result == Err(PacketError::Transaction) {
            self.observe_failure("transaction", result.err());
        }
        // Even timeout, removal and malformed FIFO observations must retire the
        // hardware channel before another caller can reuse it. Otherwise fence
        // this controller until a fresh machine-level reset/ownership epoch.
        let halted = self.io.read(STATUS) & HALTED != 0;
        let active = self.io.read(CHARACTER) & ENABLE != 0;
        if !halted || active {
            match self.halt_packet(characteristic, periodic) {
                Ok(true) if result.is_ok() => {
                    self.observe_failure("halt-rx-residue", Some(PacketError::ReceiveEnvelope));
                    result = Err(PacketError::ReceiveEnvelope);
                }
                Ok(_) => {}
                Err(()) => {
                    self.observe_failure("halt", result.err());
                    self.prepared = false;
                    return Err(PacketError::Retirement);
                }
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
                self.observe_failure("fifo-flush", result.err());
                self.prepared = false;
                return Err(PacketError::Retirement);
            }
        }
        self.io.write(STATUS, 0x7ff);
        result
    }
    fn poll_packet(
        &mut self,
        input: bool,
        start_split: bool,
        bytes: &mut [u8],
    ) -> Result<usize, PacketError> {
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
                let kind = (status >> 17) & 15;
                let data = kind == 2;
                let valid =
                    input && data && status & 15 == 0 && received.is_none() && count <= bytes.len();
                // BYTECNT describes payload only for IN-data status. Hardware
                // completion/halt entries may retain the preceding count; they
                // must not consume that many words from the FIFO a second time.
                if data {
                    for offset in (0..count).step_by(4) {
                        let word = self.io.read(FIFO).to_le_bytes();
                        if valid {
                            let take = (count - offset).min(4);
                            bytes[offset..offset + take].copy_from_slice(&word[..take]);
                        }
                    }
                }
                if (data && !valid) || (!data && (status & 15 != 0 || !matches!(kind, 3 | 5 | 7))) {
                    self.observe_receive_failure(
                        "rx-envelope",
                        Some(PacketError::ReceiveEnvelope),
                        Some(status),
                    );
                    return Err(PacketError::ReceiveEnvelope);
                }
                if data {
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
            if start_split && status & (1 << 5) != 0 {
                return Ok(0);
            }
            if status & 1 != 0 {
                // PIO OUT completion accounts for the submitted packet.
                // HCTSIZ.XFERSIZE can retain the programmed byte count.
                return if input {
                    if let Some(count) = received {
                        Ok(count)
                    } else {
                        self.observe_failure("rx-missing", Some(PacketError::ReceiveEnvelope));
                        Err(PacketError::ReceiveEnvelope)
                    }
                } else {
                    Ok(bytes.len())
                };
            }
            if status & HALTED != 0 {
                return Err(PacketError::Transaction);
            }
        }
        Err(PacketError::WorkPressure)
    }
}
#[cfg(test)]
mod tests;
