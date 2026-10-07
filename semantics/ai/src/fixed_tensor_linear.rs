//! Unbiased scalar reference projection over an admitted exact tensor resource.
use crate::fixed_neural::FixedNumericRefusal;
use crate::fixed_tensor::{FixedMatrixOrder, FixedTensorRefusal};
use conduit_data::{tensor_content_digest, TensorElement, TensorValue};

/// Borrows immutable packed little-endian f32 content without copying a model.
pub struct FixedTensorLinear<'a, const INPUT: usize, const OUTPUT: usize> {
    tensor: &'a TensorValue,
    bytes: &'a [u8],
    order: FixedMatrixOrder,
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedTensorLinear<'a, INPUT, OUTPUT> {
    pub fn prepare(
        tensor: &'a TensorValue,
        bytes: &'a [u8],
        order: FixedMatrixOrder,
    ) -> Result<Self, FixedTensorRefusal> {
        if INPUT == 0 || OUTPUT == 0 {
            return Err(FixedTensorRefusal::Numeric(FixedNumericRefusal::EmptyShape));
        }
        tensor.validate().map_err(FixedTensorRefusal::Tensor)?;
        if tensor.element != TensorElement::F32 {
            return Err(FixedTensorRefusal::Element);
        }
        let shape = match order {
            FixedMatrixOrder::InputMajor => [INPUT as u64, OUTPUT as u64],
            FixedMatrixOrder::OutputMajor => [OUTPUT as u64, INPUT as u64],
        };
        if tensor.dimensions.as_slice() != shape {
            return Err(FixedTensorRefusal::Shape);
        }
        if tensor.byte_count().map_err(FixedTensorRefusal::Tensor)? != bytes.len() as u64
            || tensor.content_digest != tensor_content_digest(bytes)
        {
            return Err(FixedTensorRefusal::ContentIdentity);
        }
        if bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|b| !read(b).is_finite())
        {
            return Err(FixedTensorRefusal::Numeric(
                FixedNumericRefusal::NonfiniteWeight,
            ));
        }
        Ok(Self {
            tensor,
            bytes,
            order,
        })
    }
    pub fn tensor(&self) -> &'a TensorValue {
        self.tensor
    }
    pub fn apply(
        &self,
        input: &[f32; INPUT],
        output: &mut [f32; OUTPUT],
    ) -> Result<(), FixedNumericRefusal> {
        if input.iter().any(|v| !v.is_finite()) {
            return Err(FixedNumericRefusal::NonfiniteInput);
        }
        let mut staged = [0.0; OUTPUT];
        for (row, result) in staged.iter_mut().enumerate() {
            let mut sum = 0.0_f32;
            for (column, value) in input.iter().enumerate() {
                let index = match self.order {
                    FixedMatrixOrder::InputMajor => column * OUTPUT + row,
                    FixedMatrixOrder::OutputMajor => row * INPUT + column,
                };
                sum += read(&self.bytes[index * 4..index * 4 + 4]) * value;
            }
            if !sum.is_finite() {
                return Err(FixedNumericRefusal::NonfiniteOutput);
            }
            *result = sum;
        }
        *output = staged;
        Ok(())
    }
}
fn read(bytes: &[u8]) -> f32 {
    f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
