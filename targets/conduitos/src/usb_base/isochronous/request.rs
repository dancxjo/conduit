//! Prepared decoding borrows canonical input and copies only eight fixed packets.
use super::contract::{
    ISOCHRONOUS_OUT_MAXIMUM_PACKETS, ISOCHRONOUS_OUT_PACKET_BYTES, IsochronousOutContract,
};
use crate::usb_base::isochronous_schedule::{IsochronousFrame, IsochronousScheduleRefusal};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, validate_canonical_structured_value,
};

#[derive(Debug, PartialEq, Eq)]
pub enum IsochronousRequestRefusal {
    Canonical(StructuredInfoRefusal),
    PacketCount,
    PacketLength,
    Frame,
    Cadence,
    Schedule(IsochronousScheduleRefusal),
}
pub struct DecodedIsochronousBatch {
    frames: [u16; ISOCHRONOUS_OUT_MAXIMUM_PACKETS],
    lengths: [u16; ISOCHRONOUS_OUT_MAXIMUM_PACKETS],
    buffers: [[u8; ISOCHRONOUS_OUT_PACKET_BYTES as usize]; ISOCHRONOUS_OUT_MAXIMUM_PACKETS],
    count: usize,
}
impl DecodedIsochronousBatch {
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn packet(&self, index: usize) -> Option<(IsochronousFrame, &[u8])> {
        if index >= self.count {
            return None;
        }
        Some((
            IsochronousFrame::new(self.frames[index]).expect("decoded 11-bit frame"),
            &self.buffers[index][..usize::from(self.lengths[index])],
        ))
    }
    /// Check every explicit deadline against one actual native clock observation
    /// before publishing the first packet. Native publication rechecks its clock.
    pub fn validate_schedule(
        &self,
        mfindex: u16,
        ist: u8,
    ) -> Result<(), IsochronousRequestRefusal> {
        for index in 0..self.count {
            self.packet(index)
                .expect("bounded packet")
                .0
                .validate(mfindex, ist)
                .map_err(IsochronousRequestRefusal::Schedule)?;
        }
        Ok(())
    }
}
#[allow(clippy::large_enum_variant)] // One preparation-reserved request owns exactly eight fixed packet buffers.
pub enum DecodedIsochronousRequest {
    Observe,
    Batch(DecodedIsochronousBatch),
    Drain,
}
pub struct PreparedIsochronousRequestDecoder {
    validator: PreparedStructuredValueValidator,
}
impl PreparedIsochronousRequestDecoder {
    pub fn new(contract: &IsochronousOutContract) -> Result<Self, StructuredInfoRefusal> {
        Ok(Self {
            validator: contract.request_validator()?,
        })
    }
    pub fn decode(
        &self,
        input: &[u8],
        maximum_packet: u16,
    ) -> Result<DecodedIsochronousRequest, IsochronousRequestRefusal> {
        use IsochronousRequestRefusal as Error;
        if maximum_packet == 0 || maximum_packet > ISOCHRONOUS_OUT_PACKET_BYTES {
            return Err(Error::PacketLength);
        }
        self.validator.validate(input).map_err(Error::Canonical)?;
        let value = validate_canonical_structured_value(input).map_err(Error::Canonical)?;
        match value.variant_tag().map_err(Error::Canonical)? {
            "observe" => Ok(DecodedIsochronousRequest::Observe),
            "drain" => Ok(DecodedIsochronousRequest::Drain),
            "batch" => {
                let payload = value
                    .variant_payload("batch")
                    .map_err(Error::Canonical)?
                    .ok_or(Error::Frame)?;
                let packets = payload
                    .record_field("packets")
                    .map_err(Error::Canonical)?
                    .ok_or(Error::PacketCount)?;
                let count = packets.collection_length().map_err(Error::Canonical)? as usize;
                if count == 0 || count > ISOCHRONOUS_OUT_MAXIMUM_PACKETS {
                    return Err(Error::PacketCount);
                }
                let mut batch = DecodedIsochronousBatch {
                    frames: [0; 8],
                    lengths: [0; 8],
                    buffers: [[0; 256]; 8],
                    count,
                };
                for index in 0..count {
                    let packet = packets
                        .collection_index(index as u16)
                        .map_err(Error::Canonical)?
                        .ok_or(Error::PacketCount)?;
                    let frame = packet
                        .record_field("frame")
                        .map_err(Error::Canonical)?
                        .ok_or(Error::Frame)?
                        .primitive_bytes("value/u64")
                        .map_err(Error::Canonical)?;
                    let frame = u64::from_le_bytes(frame.try_into().map_err(|_| Error::Frame)?);
                    let frame = u16::try_from(frame).map_err(|_| Error::Frame)?;
                    IsochronousFrame::new(frame).map_err(|_| Error::Frame)?;
                    if index != 0 && frame != ((batch.frames[index - 1] + 1) & 2047) {
                        return Err(Error::Cadence);
                    }
                    let wire = packet
                        .record_field("wire")
                        .map_err(Error::Canonical)?
                        .ok_or(Error::PacketLength)?
                        .primitive_bytes("value/bytes")
                        .map_err(Error::Canonical)?;
                    if wire.len() > usize::from(maximum_packet) {
                        return Err(Error::PacketLength);
                    }
                    batch.frames[index] = frame;
                    batch.lengths[index] = wire.len() as u16;
                    batch.buffers[index][..wire.len()].copy_from_slice(wire);
                }
                Ok(DecodedIsochronousRequest::Batch(batch))
            }
            _ => Err(Error::Canonical(StructuredInfoRefusal::UnknownVariantTag)),
        }
    }
}
