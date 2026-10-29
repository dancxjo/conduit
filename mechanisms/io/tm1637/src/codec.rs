//! Allocation-free validation of the existing authored packet ABI.
use crate::{WriteContract, MAXIMUM_BYTES};
use conduit_core as c;
#[derive(Debug, PartialEq, Eq)]
pub enum PacketRefusal {
    Canonical(c::StructuredInfoRefusal),
    Geometry,
}
impl From<c::StructuredInfoRefusal> for PacketRefusal {
    fn from(value: c::StructuredInfoRefusal) -> Self {
        Self::Canonical(value)
    }
}
pub(crate) struct DecodedPackets {
    wires: [[u8; 7]; 3],
    lengths: [u8; 3],
}
impl DecodedPackets {
    pub(crate) fn packets(&self) -> [crate::Packet<'_>; 3] {
        core::array::from_fn(|index| crate::Packet {
            bytes: &self.wires[index][..usize::from(self.lengths[index])],
        })
    }
}
pub(crate) struct PreparedPackets(c::PreparedStructuredValueValidator);
impl PreparedPackets {
    pub(crate) fn prepare(contract: &WriteContract) -> Result<Self, c::StructuredInfoRefusal> {
        Ok(Self(
            c::PreparedStructuredValueValidator::with_static_schema(
                contract.packets_type(),
                crate::contract::PACKETS_SCHEMA,
                MAXIMUM_BYTES as usize,
            )?,
        ))
    }
    pub(crate) fn decode(&self, bytes: &[u8]) -> Result<DecodedPackets, PacketRefusal> {
        self.0.validate(bytes)?;
        let value = c::validate_canonical_structured_value(bytes)?;
        let packets = value
            .record_field("packets")?
            .ok_or(PacketRefusal::Geometry)?;
        if packets.collection_length()? != 3 {
            return Err(PacketRefusal::Geometry);
        }
        let mut result = DecodedPackets {
            wires: [[0; 7]; 3],
            lengths: [0; 3],
        };
        for index in 0..3 {
            let packet = packets
                .collection_index(index as u16)?
                .ok_or(PacketRefusal::Geometry)?;
            let length = packet
                .record_field("length")?
                .ok_or(PacketRefusal::Geometry)?;
            let [length] = length.primitive_bytes("value/u8")? else {
                return Err(PacketRefusal::Geometry);
            };
            if !(1..=7).contains(length) {
                return Err(PacketRefusal::Geometry);
            }
            let wire = packet
                .record_field("wire")?
                .ok_or(PacketRefusal::Geometry)?;
            if wire.copy_octet_collection("value/u8", &mut result.wires[index])? != 7
                || result.wires[index][usize::from(*length)..]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return Err(PacketRefusal::Geometry);
            }
            result.lengths[index] = *length;
        }
        Ok(result)
    }
}
