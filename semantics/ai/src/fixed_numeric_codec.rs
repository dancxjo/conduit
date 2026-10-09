//! Allocation-prepared exact fixed-vector canonical codec.
//! Runtime checks the complete envelope and never allocates or guesses shapes.
use alloc::{vec, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape as Shape, StructuredInfoValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedCodecRefusal {
    Shape,
    Encoding,
    Nonfinite,
}

pub struct FixedF32VectorCodec<const WIDTH: usize> {
    template: Vec<u8>,
    offsets: [usize; WIDTH],
    output: Vec<u8>,
}
impl<const WIDTH: usize> FixedF32VectorCodec<WIDTH> {
    pub fn prepare(value_type: &StructuredInfoType) -> Result<Self, FixedCodecRefusal> {
        if WIDTH == 0 || WIDTH > 1024 {
            return Err(FixedCodecRefusal::Shape);
        }
        // Restrict preparation to the owned finite-only scalar contract.
        // Accepting arbitrary refined F32 profiles would weaken their invariants.
        let name = alloc::format!("NumericF32Vector{WIDTH}");
        if crate::fixed_numeric_catalog::fixed_numeric_type(&name)
            .map_err(|_| FixedCodecRefusal::Shape)?
            != *value_type
        {
            return Err(FixedCodecRefusal::Shape);
        }
        let template = vector(value_type, WIDTH, 0.0)?
            .canonical_bytes()
            .map_err(|_| FixedCodecRefusal::Encoding)?;
        // The nonzero byte pattern identifies canonical payload positions using
        // the core encoder during preparation. No wire-format offsets are guessed.
        let marked = vector(value_type, WIDTH, f32::from_bits(0x3f01_0203))?
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
                || cursor + 4 > template.len()
                || marked[cursor..cursor + 4] != [3, 2, 1, 63]
                || template[cursor..cursor + 4] != [0; 4]
            {
                return Err(FixedCodecRefusal::Encoding);
            }
            offsets[count] = cursor;
            count += 1;
            cursor += 4;
        }
        if count != WIDTH {
            return Err(FixedCodecRefusal::Encoding);
        }
        let output = template.clone();
        Ok(Self {
            template,
            offsets,
            output,
        })
    }
    pub fn decode(&self, bytes: &[u8], output: &mut [f32; WIDTH]) -> Result<(), FixedCodecRefusal> {
        if bytes.len() != self.template.len() {
            return Err(FixedCodecRefusal::Encoding);
        }
        let mut staged = [0.; WIDTH];
        let mut previous = 0;
        for (value, offset) in staged.iter_mut().zip(self.offsets) {
            if bytes[previous..offset] != self.template[previous..offset] {
                return Err(FixedCodecRefusal::Encoding);
            }
            *value = f32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .map_err(|_| FixedCodecRefusal::Encoding)?,
            );
            if !value.is_finite() {
                return Err(FixedCodecRefusal::Nonfinite);
            }
            previous = offset + 4;
        }
        if bytes[previous..] != self.template[previous..] {
            return Err(FixedCodecRefusal::Encoding);
        }
        *output = staged;
        Ok(())
    }
    pub fn encode(&mut self, values: &[f32; WIDTH]) -> Result<&[u8], FixedCodecRefusal> {
        if values.iter().any(|v| !v.is_finite()) {
            return Err(FixedCodecRefusal::Nonfinite);
        }
        for (value, offset) in values.iter().zip(self.offsets) {
            self.output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        Ok(&self.output)
    }
    pub fn encoded(&self) -> &[u8] {
        &self.output
    }
    pub fn maximum_bytes(&self) -> usize {
        self.output.len()
    }
    pub fn allocation_capacity(&self) -> usize {
        self.template.capacity() + self.output.capacity()
    }
}
fn vector(
    ty: &StructuredInfoType,
    width: usize,
    value: f32,
) -> Result<StructuredInfoValue, FixedCodecRefusal> {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), vector(representation, width, value)?)
        }
        Shape::Collection { element, length } if length as usize == width => {
            let values = (0..width)
                .map(|_| scalar(element, value))
                .collect::<Result<Vec<_>, _>>()?;
            StructuredInfoValue::collection(ty.clone(), values)
        }
        _ => return Err(FixedCodecRefusal::Shape),
    }
    .map_err(|_| FixedCodecRefusal::Encoding)
}
fn scalar(ty: &StructuredInfoType, value: f32) -> Result<StructuredInfoValue, FixedCodecRefusal> {
    match ty.shape() {
        Shape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), scalar(representation, value)?)
        }
        Shape::Leaf(kind) if kind.as_str() == conduit_core::F32_INFO_ID => {
            StructuredInfoValue::leaf(
                ty.clone(),
                vec![
                    value.to_le_bytes()[0],
                    value.to_le_bytes()[1],
                    value.to_le_bytes()[2],
                    value.to_le_bytes()[3],
                ],
            )
        }
        _ => return Err(FixedCodecRefusal::Shape),
    }
    .map_err(|_| FixedCodecRefusal::Encoding)
}
