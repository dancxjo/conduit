//! Prepared finite-only F32 envelope admission for exact numeric typed pairs.
use crate::fixed_numeric_codec::FixedCodecRefusal as Refusal;
use alloc::vec::Vec;
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape as Shape, StructuredInfoValue,
};
pub(crate) struct FiniteEnvelope {
    template: Vec<u8>,
    offsets: Vec<usize>,
}
impl FiniteEnvelope {
    pub(crate) fn prepare(ty: &StructuredInfoType) -> Result<Self, Refusal> {
        let template = tree(ty, 0.0)?
            .canonical_bytes()
            .map_err(|_| Refusal::Encoding)?;
        let marked = tree(ty, f32::from_bits(0x3f010203))?
            .canonical_bytes()
            .map_err(|_| Refusal::Encoding)?;
        let mut offsets = Vec::new();
        let mut cursor = 0;
        while cursor < template.len() {
            if template[cursor] == marked[cursor] {
                cursor += 1;
                continue;
            }
            if cursor + 4 > template.len()
                || template[cursor..cursor + 4] != [0; 4]
                || marked[cursor..cursor + 4] != [3, 2, 1, 63]
            {
                return Err(Refusal::Encoding);
            }
            offsets.push(cursor);
            cursor += 4;
        }
        Ok(Self { template, offsets })
    }
    pub(crate) fn validate(&self, bytes: &[u8]) -> Result<(), Refusal> {
        if bytes.len() != self.template.len() {
            return Err(Refusal::Encoding);
        }
        let mut previous = 0;
        for &offset in &self.offsets {
            if bytes[previous..offset] != self.template[previous..offset] {
                return Err(Refusal::Encoding);
            }
            if !f32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .map_err(|_| Refusal::Encoding)?,
            )
            .is_finite()
            {
                return Err(Refusal::Nonfinite);
            }
            previous = offset + 4;
        }
        if bytes[previous..] != self.template[previous..] {
            return Err(Refusal::Encoding);
        }
        Ok(())
    }
}
fn tree(ty: &StructuredInfoType, value: f32) -> Result<StructuredInfoValue, Refusal> {
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
        Shape::Record { fields, .. } => StructuredInfoValue::record(
            ty.clone(),
            fields
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(f.name(), tree(f.value_type(), value)?)
                        .map_err(|_| Refusal::Encoding)
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Shape::Leaf(kind) if kind.as_str() == conduit_core::F32_INFO_ID => {
            StructuredInfoValue::leaf(ty.clone(), value.to_le_bytes().to_vec())
        }
        _ => return Err(Refusal::Shape),
    }
    .map_err(|_| Refusal::Encoding)
}

impl FiniteEnvelope {
    /// Retained requested local payload capacity. Shared tensor/model Arc owners,
    /// Box root, Arc headers, allocator bookkeeping and stack are separate charges.
    pub fn local_accounted_heap_bytes(&self) -> usize {
        self.template.capacity().saturating_add(
            self.offsets
                .capacity()
                .saturating_mul(core::mem::size_of::<usize>()),
        )
    }
}
