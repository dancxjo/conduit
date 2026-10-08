//! Preparation-derived exact integer envelopes for generic index operands.
use crate::fixed_numeric_codec::FixedCodecRefusal;
use alloc::{format, vec, vec::Vec};
use conduit_core::{
    StructuredInfoType, StructuredInfoTypeShape as Shape, StructuredInfoValue, kind_id,
};
pub struct FixedU16IndexCodec<const WIDTH: usize> {
    template: Vec<u8>,
    offsets: [usize; WIDTH],
    output: Vec<u8>,
}
impl<const WIDTH: usize> FixedU16IndexCodec<WIDTH> {
    pub fn prepare(ty: &StructuredInfoType) -> Result<Self, FixedCodecRefusal> {
        let expected = if WIDTH == 1 {
            StructuredInfoType::leaf(kind_id("value/u16")).map_err(|_| FixedCodecRefusal::Shape)?
        } else {
            crate::fixed_numeric_catalog::fixed_numeric_type(&format!("NumericU16Indices{WIDTH}"))
                .map_err(|_| FixedCodecRefusal::Shape)?
        };
        if *ty != expected {
            return Err(FixedCodecRefusal::Shape);
        }
        // The portable U16 leaf port carries the primitive owner's two-byte
        // little-endian value. Structured collection ports carry the full
        // canonical structured envelope prepared below.
        if WIDTH == 1 {
            return Ok(Self {
                template: vec![0, 0],
                offsets: [0; WIDTH],
                output: vec![0, 0],
            });
        }
        let template = tree(ty, 0)?
            .canonical_bytes()
            .map_err(|_| FixedCodecRefusal::Encoding)?;
        let marked = tree(ty, 0x0201)?
            .canonical_bytes()
            .map_err(|_| FixedCodecRefusal::Encoding)?;
        if template.len() != marked.len() {
            return Err(FixedCodecRefusal::Encoding);
        }
        let mut offsets = [0; WIDTH];
        let mut count = 0;
        let mut cursor = 0;
        while cursor < template.len() {
            if template[cursor] == marked[cursor] {
                cursor += 1;
                continue;
            }
            if count == WIDTH
                || cursor + 2 > template.len()
                || marked[cursor..cursor + 2] != [1, 2]
                || template[cursor..cursor + 2] != [0, 0]
            {
                return Err(FixedCodecRefusal::Encoding);
            }
            offsets[count] = cursor;
            count += 1;
            cursor += 2;
        }
        if count != WIDTH {
            return Err(FixedCodecRefusal::Shape);
        }
        Ok(Self {
            output: template.clone(),
            template,
            offsets,
        })
    }
    pub fn decode(&self, bytes: &[u8], out: &mut [u16; WIDTH]) -> Result<(), FixedCodecRefusal> {
        if bytes.len() != self.template.len() {
            return Err(FixedCodecRefusal::Encoding);
        }
        let mut staged = [0; WIDTH];
        let mut cursor = 0;
        for (value, offset) in staged.iter_mut().zip(self.offsets) {
            if bytes[cursor..offset] != self.template[cursor..offset] {
                return Err(FixedCodecRefusal::Encoding);
            }
            *value = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            cursor = offset + 2;
        }
        if bytes[cursor..] != self.template[cursor..] {
            return Err(FixedCodecRefusal::Encoding);
        }
        *out = staged;
        Ok(())
    }
    pub fn encode(&mut self, values: &[u16; WIDTH]) -> &[u8] {
        for (value, offset) in values.iter().zip(self.offsets) {
            self.output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        &self.output
    }
}
fn tree(ty: &StructuredInfoType, value: u16) -> Result<StructuredInfoValue, FixedCodecRefusal> {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), tree(representation, value)?)
        }
        Shape::Collection { element, length } => StructuredInfoValue::collection(
            ty.clone(),
            (0..length)
                .map(|_| tree(element, value))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Shape::Leaf(kind) if kind.as_str() == "value/u16" => StructuredInfoValue::leaf(
            ty.clone(),
            vec![value.to_le_bytes()[0], value.to_le_bytes()[1]],
        ),
        _ => return Err(FixedCodecRefusal::Shape),
    }
    .map_err(|_| FixedCodecRefusal::Encoding)
}
