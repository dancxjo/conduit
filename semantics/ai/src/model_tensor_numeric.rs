//! Actual inline numeric contents, after dtype/shape/byte validation.
//! A descriptor grants no resource access and cannot prove finite float values.
use crate::{ModelTensorIoRefusal, ModelTensorNumericPolicy};
use conduit_data::TensorElement;

pub(crate) fn validate(
    policy: ModelTensorNumericPolicy,
    element: TensorElement,
    inline: Option<&[u8]>,
) -> Result<(), ModelTensorIoRefusal> {
    match policy {
        ModelTensorNumericPolicy::Ieee => return Ok(()),
        ModelTensorNumericPolicy::FiniteOnly => {}
    }
    let (width, exponent_mask) = match element {
        TensorElement::F32 => (4, 0x7f80_0000_u64),
        TensorElement::F64 => (8, 0x7ff0_0000_0000_0000_u64),
        // Every supported integer bit pattern represents a finite integer,
        // including resource-backed values; no contents need be opened.
        TensorElement::I8
        | TensorElement::U8
        | TensorElement::I16
        | TensorElement::U16
        | TensorElement::I24
        | TensorElement::I32
        | TensorElement::U32
        | TensorElement::I64
        | TensorElement::U64 => return Ok(()),
    };
    let bytes = inline.ok_or(ModelTensorIoRefusal::NumericPolicyUnverifiable)?;
    let mut chunks = bytes.chunks_exact(width);
    for chunk in &mut chunks {
        let bits = if width == 4 {
            u64::from(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        } else {
            u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ])
        };
        if bits & exponent_mask == exponent_mask {
            return Err(ModelTensorIoRefusal::NonFinite);
        }
    }
    if !chunks.remainder().is_empty() {
        return Err(ModelTensorIoRefusal::ByteBound);
    }
    Ok(())
}
