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
const PACKETS_TYPE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/packets.type"));
// The independently checked immutable schema is linked once, rather than
// retained as a second decoded Type and canonical prefix in every selected owner.
pub(crate) struct PreparedPackets {
    _checked: (),
}
impl PreparedPackets {
    pub(crate) fn prepare(contract: &WriteContract) -> Result<Self, c::StructuredInfoRefusal> {
        if MAXIMUM_BYTES == 0
            || MAXIMUM_BYTES as usize > c::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || PACKETS_TYPE.len() >= MAXIMUM_BYTES as usize
        {
            return Err(c::StructuredInfoRefusal::CanonicalEncodingTooLarge);
        }
        if contract.packets_type().canonical_bytes()?.as_slice() != PACKETS_TYPE {
            return Err(c::StructuredInfoRefusal::WrongType);
        }
        Ok(Self { _checked: () })
    }
    pub(crate) fn decode(&self, bytes: &[u8]) -> Result<DecodedPackets, PacketRefusal> {
        if bytes.len() > MAXIMUM_BYTES as usize {
            return Err(c::StructuredInfoRefusal::CanonicalEncodingTooLarge.into());
        }
        if !bytes.starts_with(PACKETS_TYPE) {
            return Err(c::StructuredInfoRefusal::WrongType.into());
        }
        let value = c::validate_canonical_structured_value(bytes)?;
        if value.type_bytes() != PACKETS_TYPE {
            return Err(c::StructuredInfoRefusal::WrongType.into());
        }
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
