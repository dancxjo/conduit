//! Prior scalar traversal and borrowed/owned view shape, benchmark-only.
//! Both paths share the same validated tensor content; this mints no authority.
use conduit_ai::{
    fixed_neural::FixedNumericRefusal, fixed_tensor::FixedMatrixOrder,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use conduit_data::TensorValue;
use std::sync::Arc;
enum View<'a> {
    Borrowed {
        tensor: &'a TensorValue,
        bytes: &'a [u8],
    },
    Owned(Arc<AdmittedFixedTensorResource>),
}
impl View<'_> {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Borrowed { bytes, .. } => bytes,
            Self::Owned(resource) => resource.bytes(),
        }
    }
}
pub(super) struct Reference<'a> {
    view: View<'a>,
    order: FixedMatrixOrder,
}
impl<'a> Reference<'a> {
    pub(super) fn new(tensor: &'a TensorValue, bytes: &'a [u8], order: FixedMatrixOrder) -> Self {
        Self {
            view: View::Borrowed { tensor, bytes },
            order,
        }
    }
    #[inline(never)]
    pub(super) fn apply<const I: usize, const O: usize>(
        &self,
        input: &[f32; I],
        output: &mut [f32; O],
    ) -> Result<(), FixedNumericRefusal> {
        if input.iter().any(|v| !v.is_finite()) {
            return Err(FixedNumericRefusal::NonfiniteInput);
        }
        let bytes = self.view.bytes();
        let mut staged = [0.; O];
        for (row, result) in staged.iter_mut().enumerate() {
            let mut sum = 0.0_f32;
            for (column, value) in input.iter().enumerate() {
                let index = match self.order {
                    FixedMatrixOrder::InputMajor => column * O + row,
                    FixedMatrixOrder::OutputMajor => row * I + column,
                };
                let b = &bytes[index * 4..index * 4 + 4];
                sum += f32::from_le_bytes([b[0], b[1], b[2], b[3]]) * value;
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
