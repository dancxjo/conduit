//! Fixed affine reference computation over admitted Data tensor content.
//! Resource access/authority and ModelArtifact admission belong to the caller.
use crate::fixed_neural::FixedNumericRefusal;
#[cfg(target_has_atomic = "ptr")]
use crate::fixed_tensor_resource::AdmittedFixedTensorResource;
use crate::fixed_tensor_resource::FixedTensorView;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use conduit_data::{tensor_content_digest, TensorElement, TensorRefusal, TensorValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedMatrixOrder {
    OutputMajor,
    InputMajor,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedTensorRefusal {
    Tensor(TensorRefusal),
    Shape,
    Element,
    ContentIdentity,
    Numeric(FixedNumericRefusal),
}

/// Packed little-endian f32 weights, without unsafe alignment assumptions or a
/// second copied model. Shape, packing order and exact content remain explicit.
pub struct FixedTensorAffine<'a, const INPUT: usize, const OUTPUT: usize> {
    weights: FixedTensorView<'a>,
    bias: FixedTensorView<'a>,
    order: FixedMatrixOrder,
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedTensorAffine<'a, INPUT, OUTPUT> {
    pub fn prepare(
        weights: &'a TensorValue,
        weight_bytes: &'a [u8],
        bias: &'a TensorValue,
        bias_bytes: &'a [u8],
        order: FixedMatrixOrder,
    ) -> Result<Self, FixedTensorRefusal> {
        Self::prepare_views(
            FixedTensorView::borrowed(weights, weight_bytes),
            FixedTensorView::borrowed(bias, bias_bytes),
            order,
        )
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn prepare_owned(
        weights: Arc<AdmittedFixedTensorResource>,
        bias: Arc<AdmittedFixedTensorResource>,
        order: FixedMatrixOrder,
    ) -> Result<Self, FixedTensorRefusal> {
        Self::prepare_views(
            FixedTensorView::Owned(weights),
            FixedTensorView::Owned(bias),
            order,
        )
    }
    fn prepare_views(
        weights: FixedTensorView<'a>,
        bias: FixedTensorView<'a>,
        order: FixedMatrixOrder,
    ) -> Result<Self, FixedTensorRefusal> {
        let weight_bytes = weights.bytes();
        let bias_bytes = bias.bytes();
        if INPUT == 0 || OUTPUT == 0 {
            return Err(FixedTensorRefusal::Numeric(FixedNumericRefusal::EmptyShape));
        }
        let shape = match order {
            FixedMatrixOrder::OutputMajor => [OUTPUT as u64, INPUT as u64],
            FixedMatrixOrder::InputMajor => [INPUT as u64, OUTPUT as u64],
        };
        validate_tensor(weights.tensor(), weight_bytes, &shape)?;
        validate_tensor(bias.tensor(), bias_bytes, &[OUTPUT as u64])?;
        if weight_bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|bytes| !read_f32(bytes).is_finite())
        {
            return Err(FixedTensorRefusal::Numeric(
                FixedNumericRefusal::NonfiniteWeight,
            ));
        }
        if bias_bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|bytes| !read_f32(bytes).is_finite())
        {
            return Err(FixedTensorRefusal::Numeric(
                FixedNumericRefusal::NonfiniteBias,
            ));
        }
        Ok(Self {
            weights,
            bias,
            order,
        })
    }
    pub fn weights(&self) -> &TensorValue {
        self.weights.tensor()
    }
    pub fn bias(&self) -> &TensorValue {
        self.bias.tensor()
    }
    pub fn order(&self) -> FixedMatrixOrder {
        self.order
    }

    /// The same scalar f32 accumulation law as FixedAffine. No layer ordering,
    /// activation or recurrent update is performed here.
    pub fn apply(
        &self,
        input: &[f32; INPUT],
        output: &mut [f32; OUTPUT],
    ) -> Result<(), FixedNumericRefusal> {
        if input.iter().any(|value| !value.is_finite()) {
            return Err(FixedNumericRefusal::NonfiniteInput);
        }
        let weight_bytes = self.weights.bytes();
        let bias_bytes = self.bias.bytes();
        let mut staged = [0.0; OUTPUT];
        for (row, result) in staged.iter_mut().enumerate() {
            let mut sum = read_f32(&bias_bytes[row * 4..row * 4 + 4]);
            for (column, value) in input.iter().enumerate() {
                let index = match self.order {
                    FixedMatrixOrder::OutputMajor => row * INPUT + column,
                    FixedMatrixOrder::InputMajor => column * OUTPUT + row,
                };
                sum += read_f32(&weight_bytes[index * 4..index * 4 + 4]) * value;
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
/// An exact resource-backed row-major table. Index policy belongs to the
/// authored caller; this primitive only admits and copies a generic row.
pub struct FixedTensorEmbedding<'a, const ROWS: usize, const WIDTH: usize> {
    view: FixedTensorView<'a>,
}
impl<'a, const ROWS: usize, const WIDTH: usize> FixedTensorEmbedding<'a, ROWS, WIDTH> {
    pub fn prepare(tensor: &'a TensorValue, bytes: &'a [u8]) -> Result<Self, FixedTensorRefusal> {
        Self::prepare_view(FixedTensorView::borrowed(tensor, bytes))
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn prepare_owned(
        resource: Arc<AdmittedFixedTensorResource>,
    ) -> Result<Self, FixedTensorRefusal> {
        Self::prepare_view(FixedTensorView::Owned(resource))
    }
    fn prepare_view(view: FixedTensorView<'a>) -> Result<Self, FixedTensorRefusal> {
        let tensor = view.tensor();
        let bytes = view.bytes();
        if ROWS == 0 || WIDTH == 0 {
            return Err(FixedTensorRefusal::Numeric(FixedNumericRefusal::EmptyShape));
        }
        validate_tensor(tensor, bytes, &[ROWS as u64, WIDTH as u64])?;
        if bytes
            .as_chunks::<4>()
            .0
            .iter()
            .any(|value| !read_f32(value).is_finite())
        {
            return Err(FixedTensorRefusal::Numeric(
                FixedNumericRefusal::NonfiniteWeight,
            ));
        }
        Ok(Self { view })
    }
    pub fn tensor(&self) -> &TensorValue {
        self.view.tensor()
    }
    pub fn lookup(
        &self,
        index: usize,
        output: &mut [f32; WIDTH],
    ) -> Result<(), FixedNumericRefusal> {
        if index >= ROWS {
            return Err(FixedNumericRefusal::Index);
        }
        let bytes = self.view.bytes();
        let mut staged = [0.0; WIDTH];
        for (column, value) in staged.iter_mut().enumerate() {
            let offset = (index * WIDTH + column) * 4;
            *value = read_f32(&bytes[offset..offset + 4]);
        }
        *output = staged;
        Ok(())
    }
}
fn validate_tensor(
    tensor: &TensorValue,
    bytes: &[u8],
    shape: &[u64],
) -> Result<(), FixedTensorRefusal> {
    tensor.validate().map_err(FixedTensorRefusal::Tensor)?;
    if tensor.element != TensorElement::F32 {
        return Err(FixedTensorRefusal::Element);
    }
    if tensor.dimensions.as_slice() != shape {
        return Err(FixedTensorRefusal::Shape);
    }
    if tensor.byte_count().map_err(FixedTensorRefusal::Tensor)? != bytes.len() as u64
        || tensor_content_digest(bytes) != tensor.content_digest
    {
        return Err(FixedTensorRefusal::ContentIdentity);
    }
    Ok(())
}
fn read_f32(bytes: &[u8]) -> f32 {
    f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
