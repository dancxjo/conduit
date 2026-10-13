//! Allocation-prepared exact signed16 fixed-vector codec. No PCM normalization policy.
use crate::fixed_numeric_codec::FixedCodecRefusal;
use alloc::{vec, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
pub struct FixedI16VectorCodec<const N: usize> {
    template: Vec<u8>,
    offsets: [usize; N],
    output: Vec<u8>,
}
impl<const N: usize> FixedI16VectorCodec<N> {
    pub fn prepare(value_type: &StructuredInfoType) -> Result<Self, FixedCodecRefusal> {
        if !matches!(N, 80 | 160)
            || crate::fixed_numeric_catalog::fixed_numeric_type(&alloc::format!(
                "NumericI16Vector{N}"
            ))
            .map_err(|_| FixedCodecRefusal::Shape)?
                != *value_type
        {
            return Err(FixedCodecRefusal::Shape);
        }
        let template = tree(value_type, 0)?
            .canonical_bytes()
            .map_err(|_| FixedCodecRefusal::Encoding)?;
        let marked = tree(value_type, 0x0102)?
            .canonical_bytes()
            .map_err(|_| FixedCodecRefusal::Encoding)?;
        if template.len() != marked.len() {
            return Err(FixedCodecRefusal::Encoding);
        }
        let mut offsets = [0; N];
        let mut count = 0;
        let mut cursor = 0;
        while cursor < template.len() {
            if template[cursor] == marked[cursor] {
                cursor += 1;
                continue;
            }
            if count == N
                || cursor + 2 > template.len()
                || marked[cursor..cursor + 2] != [2, 1]
                || template[cursor..cursor + 2] != [0, 0]
            {
                return Err(FixedCodecRefusal::Encoding);
            }
            offsets[count] = cursor;
            count += 1;
            cursor += 2;
        }
        if count != N {
            return Err(FixedCodecRefusal::Encoding);
        }
        let output = template.clone();
        Ok(Self {
            template,
            offsets,
            output,
        })
    }
    pub fn encode(&mut self, values: &[i16; N]) -> &[u8] {
        self.output.copy_from_slice(&self.template);
        for (&value, offset) in values.iter().zip(self.offsets) {
            self.output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }
        &self.output
    }
    pub fn encoded(&self) -> &[u8] {
        &self.output
    }
    pub fn decode(
        &self,
        bytes: &[u8],
        destination: &mut [i16; N],
    ) -> Result<(), FixedCodecRefusal> {
        if bytes.len() != self.template.len() {
            return Err(FixedCodecRefusal::Encoding);
        }
        let mut staged = [0; N];
        let mut previous = 0;
        for (value, offset) in staged.iter_mut().zip(self.offsets) {
            if bytes[previous..offset] != self.template[previous..offset] {
                return Err(FixedCodecRefusal::Encoding);
            }
            *value = i16::from_le_bytes(
                bytes[offset..offset + 2]
                    .try_into()
                    .map_err(|_| FixedCodecRefusal::Encoding)?,
            );
            previous = offset + 2;
        }
        if bytes[previous..] != self.template[previous..] {
            return Err(FixedCodecRefusal::Encoding);
        }
        *destination = staged;
        Ok(())
    }
}

fn tree(ty: &StructuredInfoType, value: i16) -> Result<StructuredInfoValue, FixedCodecRefusal> {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), tree(representation, value)?)
        }
        StructuredInfoTypeShape::Collection { element, length } => {
            let leaf = tree(element, value)?;
            StructuredInfoValue::collection(ty.clone(), vec![leaf; usize::from(length)])
        }
        StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/i16" => {
            StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec())
        }
        _ => return Err(FixedCodecRefusal::Shape),
    }
    .map_err(|_| FixedCodecRefusal::Encoding)
}

impl<const N: usize> FixedI16VectorCodec<N> {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.template
            .capacity()
            .saturating_add(self.output.capacity())
    }
}
