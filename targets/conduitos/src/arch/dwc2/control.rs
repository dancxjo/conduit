//! Finite USB control-transfer mechanics; setup bytes remain opaque protocol.
//! Source constructs requests and interprets replies through the existing owner.
use super::{Controller, PacketError, PacketPid, RegisterIo, RootPacket};

impl<I: RegisterIo> Controller<I> {
    /// Selected EP0 routing comes from the native attachment owner. It is not
    /// inferred from request bytes. Data length is bounded by the existing
    /// control envelope; no setup opcode or device class is interpreted here.
    /// NAK/NYET and failures return immediately without a hidden retry.
    pub fn root_control(
        &mut self,
        address: u8,
        maximum_packet: u8,
        mut setup: [u8; 8],
        output: &[u8],
        input: &mut [u8],
    ) -> Result<usize, PacketError> {
        let length = usize::from(u16::from_le_bytes([setup[6], setup[7]]));
        let incoming = setup[0] & 0x80 != 0;
        if address > 127
            || !matches!(maximum_packet, 8 | 16 | 32 | 64)
            || length > 256
            || (incoming && (!output.is_empty() || input.len() < length))
            || (!incoming && output.len() != length)
        {
            return Err(PacketError::Geometry);
        }
        let route = RootPacket {
            address,
            maximum_packet,
            pid: PacketPid::Setup,
            input: false,
        };
        self.root_packet(route, &mut setup)?;
        let mut actual = 0;
        let mut pid = PacketPid::Data1;
        // At most 32 data packets for the admitted 256-byte / 8-byte envelope.
        while actual < length {
            let remaining = length - actual;
            let mut bytes = [0; 64];
            let count = remaining.min(usize::from(maximum_packet));
            let transferred = if incoming {
                let received = self.root_packet(
                    RootPacket {
                        pid,
                        input: true,
                        ..route
                    },
                    &mut bytes[..usize::from(maximum_packet)],
                )?;
                if received > count {
                    return Err(PacketError::ReceiveEnvelope);
                }
                input[actual..actual + received].copy_from_slice(&bytes[..received]);
                received
            } else {
                bytes[..count].copy_from_slice(&output[actual..actual + count]);
                self.root_packet(RootPacket { pid, ..route }, &mut bytes[..count])?
            };
            actual += transferred;
            if incoming && transferred < usize::from(maximum_packet) {
                break;
            }
            pid = if pid == PacketPid::Data1 {
                PacketPid::Data0
            } else {
                PacketPid::Data1
            };
        }
        // No-data controls always acknowledge with an IN status packet.
        let status_in = length == 0 || !incoming;
        let mut status = [0; 64];
        let status_length = if status_in {
            usize::from(maximum_packet)
        } else {
            0
        };
        if self.root_packet(
            RootPacket {
                input: status_in,
                pid: PacketPid::Data1,
                ..route
            },
            &mut status[..status_length],
        )? != 0
        {
            return Err(PacketError::ReceiveEnvelope);
        }
        Ok(actual)
    }
}

#[cfg(test)]
mod tests;
