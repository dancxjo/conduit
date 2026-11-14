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
// The same checked build artifact defines WriteContract and this retained
// validator. Its immutable bytes live in the image rather than Play's heap.
const PACKETS_TYPE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/packets.type"));
pub(crate) struct PreparedPackets(c::BorrowedStructuredValueValidator<'static>);
impl PreparedPackets {
    pub(crate) fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    pub(crate) fn prepare(contract: &WriteContract) -> Result<Self, c::StructuredInfoRefusal> {
        // Keep the full runtime Type comparison: linking bytes is storage
        // sharing, not permission to substitute a different packet contract.
        if contract.packets_type().canonical_bytes()? != PACKETS_TYPE {
            return Err(c::StructuredInfoRefusal::WrongType);
        }
        Ok(Self(c::BorrowedStructuredValueValidator::new(
            PACKETS_TYPE,
            MAXIMUM_BYTES as usize,
        )?))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_packet_validator_keeps_exact_schema_without_heap_backing() {
        let contract = WriteContract::prepare().unwrap();
        let prepared = PreparedPackets::prepare(&contract).unwrap();
        assert_eq!(prepared.retained_backing_bytes(), Some(0));
        assert_eq!(
            prepared.0.type_bytes(),
            contract.packets_type().canonical_bytes().unwrap()
        );
        let owned = c::PreparedStructuredValueValidator::new(
            contract.packets_type(),
            MAXIMUM_BYTES as usize,
        )
        .unwrap();
        assert!(owned.retained_backing_bytes().unwrap() >= PACKETS_TYPE.len());
        assert!(matches!(
            prepared.0.validate(b"foreign schema"),
            Err(c::StructuredInfoRefusal::WrongType)
        ));
    }
}
