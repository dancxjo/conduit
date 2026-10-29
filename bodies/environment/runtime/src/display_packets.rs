//! Execute the authored TM1637 packet expression; native GPIO sees only its
//! finite packet bytes. No native command synthesis, I/O, admission or scheduler.
use conduit_core::{
    validate_canonical_structured_value as validate, PreparedStructuredComposer as C,
    StructuredInfoRefusal as R,
};
use conduit_plot::{
    PortableExpressionEvaluationRefusal, PortableExpressionProgramRefusal,
    PreparedPortableExpressionEvaluator,
};
#[path = "display_frame.rs"]
mod frame;
pub use frame::{validate_display_frame, PreparedDisplayFrame, DISPLAY_FRAME_BYTES};

#[derive(Debug, PartialEq, Eq)]
pub enum DisplayPacketRefusal {
    Program(PortableExpressionProgramRefusal),
    Evaluation(PortableExpressionEvaluationRefusal),
    Canonical(R),
    InvalidBrightness,
    PacketExtent,
}
impl From<R> for DisplayPacketRefusal {
    fn from(value: R) -> Self {
        Self::Canonical(value)
    }
}
impl From<PortableExpressionEvaluationRefusal> for DisplayPacketRefusal {
    fn from(value: PortableExpressionEvaluationRefusal) -> Self {
        Self::Evaluation(value)
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct SourceDisplayPackets {
    wires: [[u8; 7]; 3],
    lengths: [u8; 3],
}
impl SourceDisplayPackets {
    pub fn packets(&self) -> [&[u8]; 3] {
        core::array::from_fn(|i| &self.wires[i][..usize::from(self.lengths[i])])
    }
}
pub struct PreparedDisplayPackets {
    evaluator: PreparedPortableExpressionEvaluator,
    frame: PreparedDisplayFrame,
    packet_output: C,
}
impl PreparedDisplayPackets {
    pub fn prepare() -> Result<Self, DisplayPacketRefusal> {
        mod material {
            include!(concat!(env!("OUT_DIR"), "/tm1637-expression-material.rs"));
        }
        let canonical = include_bytes!(concat!(env!("OUT_DIR"), "/tm1637-packets.program"));
        let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/tm1637-expression.make"));
        if conduit_core::assigned_plan_payload_digest(canonical) != material::PROGRAM_DIGEST
            || conduit_core::assigned_plan_payload_digest(bytes) != material::MATERIAL_DIGEST
        {
            return Err(PortableExpressionEvaluationRefusal::InvalidProgram.into());
        }
        let packet_type = conduit_core::StructuredInfoType::from_canonical_bytes(include_bytes!(
            concat!(env!("OUT_DIR"), "/tm1637-packet-output.type")
        ))?;
        // SAFETY: complete canonical Source/configuration, checked lowering and
        // independent digests are linked from this same trusted Make build;
        // received frame data cannot establish or replace any of this origin.
        let evaluator = unsafe {
            PreparedPortableExpressionEvaluator::from_linked_make_with_bounds(
                bytes,
                material::PROGRAM_DIGEST,
                material::BOUNDS,
            )
        }?;
        Ok(Self {
            evaluator,
            frame: PreparedDisplayFrame::prepare()?,
            packet_output: C::new(&packet_type, 1024)?,
        })
    }
    /// Canonical packet input for the selected electrical Host Call. The bytes
    /// borrow prepared storage and must be consumed/copied before the next call.
    pub fn encode_canonical(
        &mut self,
        registers: [u8; 6],
        brightness: u8,
        enabled: bool,
    ) -> Result<&[u8], DisplayPacketRefusal> {
        let frame = self.frame.encode(registers, brightness, enabled)?;
        let result = validate(self.evaluator.evaluate(frame)?)?;
        if result.variant_payload("invalid-brightness")?.is_some() {
            return Err(DisplayPacketRefusal::InvalidBrightness);
        }
        let packets = result
            .variant_payload("packets")?
            .ok_or(R::WrongType)?
            .record_field("packets")?
            .ok_or(R::WrongType)?;
        self.packet_output.record(&[packets])?;
        Ok(self.packet_output.encoded())
    }
    /// Compatibility entrance; Frame-only producers retain PreparedDisplayFrame.
    pub fn encode_frame(
        &mut self,
        registers: [u8; 6],
        brightness: u8,
        enabled: bool,
    ) -> Result<&[u8], DisplayPacketRefusal> {
        self.frame
            .encode(registers, brightness, enabled)
            .map_err(Into::into)
    }
    pub fn encode(
        &mut self,
        registers: [u8; 6],
        brightness: u8,
        enabled: bool,
    ) -> Result<SourceDisplayPackets, DisplayPacketRefusal> {
        let canonical = self.encode_canonical(registers, brightness, enabled)?;
        let packets = validate(canonical)?
            .record_field("packets")?
            .ok_or(R::WrongType)?;
        if packets.collection_length()? != 3 {
            return Err(DisplayPacketRefusal::PacketExtent);
        }
        let mut batch = SourceDisplayPackets {
            wires: [[0; 7]; 3],
            lengths: [0; 3],
        };
        for i in 0..3 {
            let packet = packets.collection_index(i as u16)?.ok_or(R::WrongType)?;
            let length = packet.record_field("length")?.ok_or(R::WrongType)?;
            let [length] = length
                .value_node()
                .get(5..)
                .ok_or(R::MalformedCanonicalEncoding)?
            else {
                return Err(R::WrongType.into());
            };
            if !(1..=7).contains(length) {
                return Err(DisplayPacketRefusal::PacketExtent);
            }
            let count = packet
                .record_field("wire")?
                .ok_or(R::WrongType)?
                .copy_octet_collection("value/u8", &mut batch.wires[i])?;
            if count != 7
                || batch.wires[i][usize::from(*length)..]
                    .iter()
                    .any(|byte| *byte != 0)
            {
                return Err(DisplayPacketRefusal::PacketExtent);
            }
            batch.lengths[i] = *length;
        }
        Ok(batch)
    }
}
