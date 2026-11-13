//! DWC2 start/complete-split scheduling, not hub or HID class protocol.
//! A TT-accepted operation remains outstanding until its complete split settles.
use super::packet::WirePacket;
use super::{Controller, EndpointRoute, POLL_LIMIT, PacketError, PacketPid, RegisterIo};
const FRAME: usize = 0x408;
impl<I: RegisterIo> Controller<I> {
    pub fn packet_on(
        &mut self,
        route: EndpointRoute,
        pid: PacketPid,
        input: bool,
        bytes: &mut [u8],
    ) -> Result<usize, PacketError> {
        if !route.valid() {
            return Err(PacketError::Geometry);
        }
        let packet = WirePacket {
            route,
            pid,
            input,
            split: 0,
            start_split: false,
        };
        let Some(tt) = route.translator else {
            return self.wire_packet(packet, bytes);
        };
        if !self.prepared {
            return Err(PacketError::NotPrepared);
        }
        let start = self.split_start_frame()?;
        let split = (1 << 31) | (3 << 14) | (u32::from(tt.hub_address) << 7) | u32::from(tt.port);
        let out_length = bytes.len();
        // ACK means the TT accepted the start, not that the device completed it.
        match self.wire_packet(
            WirePacket {
                split,
                start_split: true,
                ..packet
            },
            bytes,
        ) {
            Ok(_) => self.quiescent = false,
            Err(error @ (PacketError::Nak | PacketError::Nyet)) => return Err(error),
            Err(_) => return self.unretired_split(),
        }
        // Complete split NYET continues this *same* TT transaction. It does not
        // resubmit a setup request or replay a device/class operation. Bound the
        // continuation to three slots in the original full-speed frame.
        for offset in 2..=4 {
            if self.wait_split_frame(start, offset).is_err() {
                return self.unretired_split();
            }
            let slice = if input { &mut *bytes } else { &mut [] };
            match self.wire_packet(
                WirePacket {
                    split: split | (1 << 16),
                    ..packet
                },
                slice,
            ) {
                Ok(actual) => return Ok(if input { actual } else { out_length }),
                Err(PacketError::Nyet) => self.quiescent = false,
                Err(error @ (PacketError::Nak | PacketError::Stall)) => return Err(error),
                Err(_) => return self.unretired_split(),
            }
        }
        self.unretired_split()
    }
    fn unretired_split<T>(&mut self) -> Result<T, PacketError> {
        // Local channel halt cannot stand in for an unknown downstream TT
        // outcome. Keep the lease fenced until an explicit physical reset.
        self.quiescent = false;
        self.prepared = false;
        Err(PacketError::Retirement)
    }
    fn split_start_frame(&mut self) -> Result<u16, PacketError> {
        let start = self.io.now_us();
        for _ in 0..POLL_LIMIT {
            let frame = self.io.read(FRAME) as u16;
            if frame & 7 == 0 {
                return Ok(frame);
            }
            if self.io.now_us().wrapping_sub(start) >= 2000 {
                return Err(PacketError::Deadline);
            }
        }
        Err(PacketError::WorkPressure)
    }
    fn wait_split_frame(&mut self, start: u16, offset: u16) -> Result<(), PacketError> {
        let clock = self.io.now_us();
        for _ in 0..POLL_LIMIT {
            let elapsed = (self.io.read(FRAME) as u16).wrapping_sub(start);
            if elapsed > 6 {
                return Err(PacketError::Deadline);
            }
            if elapsed >= offset {
                return Ok(());
            }
            if self.io.now_us().wrapping_sub(clock) >= 1000 {
                return Err(PacketError::Deadline);
            }
        }
        Err(PacketError::WorkPressure)
    }
}

#[cfg(test)]
mod tests;
