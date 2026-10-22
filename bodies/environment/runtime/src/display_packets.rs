//! Execute the authored TM1637 packet expression; native GPIO sees only its
//! finite packet bytes. No native command synthesis, I/O, admission or scheduler.
use conduit_core::{
    validate_canonical_structured_value as validate, InfoBool, PreparedStructuredComposer as C,
    StructuredInfoRefusal as R, StructuredInfoType as T, StructuredInfoTypeShape as S,
};
use conduit_plot::{
    PortableExpressionEvaluationRefusal, PortableExpressionProgram,
    PortableExpressionProgramRefusal, PreparedPortableExpressionEvaluator,
};
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
    octets: [C; 6],
    registers: C,
    brightness: C,
    enabled: C,
    frame: C,
}
impl PreparedDisplayPackets {
    pub fn prepare() -> Result<Self, DisplayPacketRefusal> {
        let program = PortableExpressionProgram::from_canonical_bytes(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/tm1637-packets.program"
        )))
        .map_err(DisplayPacketRefusal::Program)?;
        let input = &program.input_type;
        let registers = field(input, "registers")?;
        let S::Collection { element, length: 6 } = registers.shape() else {
            return Err(R::WrongType.into());
        };
        let octet = || C::new(element, 64);
        Ok(Self {
            evaluator: PreparedPortableExpressionEvaluator::new(&program)?,
            octets: [octet()?, octet()?, octet()?, octet()?, octet()?, octet()?],
            registers: C::new(registers, 512)?,
            brightness: C::new(field(input, "brightness")?, 64)?,
            enabled: C::new(field(input, "enabled")?, 64)?,
            frame: C::new(input, 1024)?,
        })
    }
    pub fn encode(
        &mut self,
        registers: [u8; 6],
        brightness: u8,
        enabled: bool,
    ) -> Result<SourceDisplayPackets, DisplayPacketRefusal> {
        for (encoder, value) in self.octets.iter_mut().zip(registers) {
            encoder.leaf(&[value])?;
        }
        self.registers.collection(&[
            validate(self.octets[0].encoded())?,
            validate(self.octets[1].encoded())?,
            validate(self.octets[2].encoded())?,
            validate(self.octets[3].encoded())?,
            validate(self.octets[4].encoded())?,
            validate(self.octets[5].encoded())?,
        ])?;
        self.brightness.leaf(&[brightness])?;
        self.enabled.leaf(&InfoBool::new(enabled).encode())?;
        self.frame.record(&[
            validate(self.brightness.encoded())?,
            validate(self.enabled.encoded())?,
            validate(self.registers.encoded())?,
        ])?;
        let result = validate(self.evaluator.evaluate(self.frame.encoded())?)?;
        if result.variant_payload("invalid-brightness")?.is_some() {
            return Err(DisplayPacketRefusal::InvalidBrightness);
        }
        let packets = result
            .variant_payload("packets")?
            .ok_or(R::WrongType)?
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
fn field<'a>(ty: &'a T, name: &str) -> Result<&'a T, R> {
    let S::Record { fields, .. } = ty.shape() else {
        return Err(R::WrongType);
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value_type())
        .ok_or(R::WrongType)
}
