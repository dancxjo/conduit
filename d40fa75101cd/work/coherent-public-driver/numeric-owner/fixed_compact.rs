//! Bounded signed Q7 quantization and an explicit packed integer linear profile.
//! Source chooses quantization, clipping, scales, bias, and network composition.
#[cfg(target_has_atomic = "ptr")]
use crate::fixed_tensor_resource::AdmittedFixedTensorResource;
use crate::fixed_tensor_resource::FixedTensorView;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use conduit_data::{tensor_content_digest, TensorElement, TensorValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactTensorRefusal {
    Shape,
    Element,
    Content,
    Nonfinite,
    Scale,
    InputDomain,
    Overflow,
}
/// Input groups of four, output groups of eight, then output lane and input lane.
/// Logical Tensor dimensions remain [inputs, outputs]; packing is explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactMatrixPacking {
    Input4Output8Tiles,
}
/// Round floor(127*x + .5) in IEEE F32, after explicit unit-domain admission.
/// Caller owns clipping; this operation refuses values outside [-1,1].
pub fn fixed_signed_q7<const WIDTH: usize>(
    input: &[f32; WIDTH],
    output: &mut [i8; WIDTH],
) -> Result<(), CompactTensorRefusal> {
    if WIDTH == 0 {
        return Err(CompactTensorRefusal::Shape);
    }
    let mut staged = [0; WIDTH];
    for (i, value) in input.iter().enumerate() {
        if !value.is_finite() {
            return Err(CompactTensorRefusal::Nonfinite);
        }
        if !(-1.0..=1.0).contains(value) {
            return Err(CompactTensorRefusal::InputDomain);
        }
        staged[i] = libm::floorf(127.0 * value + 0.5) as i8;
    }
    *output = staged;
    Ok(())
}
/// Immutable borrowed or admitted owned tensors; no model copy or alignment reliance.
pub struct FixedCompactTensorLinear<'a, const INPUT: usize, const OUTPUT: usize> {
    weights: FixedTensorView<'a>,
    scales: FixedTensorView<'a>,
    bias: Option<FixedTensorView<'a>>,
    packing: CompactMatrixPacking,
}
fn validate(
    tensor: &TensorValue,
    bytes: &[u8],
    element: TensorElement,
    dimensions: &[u64],
) -> Result<(), CompactTensorRefusal> {
    tensor
        .validate()
        .map_err(|_| CompactTensorRefusal::Content)?;
    if tensor.element != element {
        return Err(CompactTensorRefusal::Element);
    }
    if tensor.dimensions.as_slice() != dimensions {
        return Err(CompactTensorRefusal::Shape);
    }
    if tensor
        .byte_count()
        .map_err(|_| CompactTensorRefusal::Shape)?
        != bytes.len() as u64
        || tensor_content_digest(bytes) != tensor.content_digest
    {
        return Err(CompactTensorRefusal::Content);
    }
    Ok(())
}
fn f32_at(bytes: &[u8], index: usize) -> f32 {
    let start = index * 4;
    f32::from_le_bytes(
        bytes[start..start + 4]
            .try_into()
            .expect("admitted F32 extent"),
    )
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedCompactTensorLinear<'a, INPUT, OUTPUT> {
    pub fn prepare(
        weights: &'a TensorValue,
        weight_bytes: &'a [u8],
        scales: &'a TensorValue,
        scale_bytes: &'a [u8],
        bias: &'a TensorValue,
        bias_bytes: &'a [u8],
        packing: CompactMatrixPacking,
    ) -> Result<Self, CompactTensorRefusal> {
        Self::prepare_fields(
            weights,
            weight_bytes,
            scales,
            scale_bytes,
            Some((bias, bias_bytes)),
            packing,
        )
    }
    pub fn prepare_unbiased(
        weights: &'a TensorValue,
        weight_bytes: &'a [u8],
        scales: &'a TensorValue,
        scale_bytes: &'a [u8],
        packing: CompactMatrixPacking,
    ) -> Result<Self, CompactTensorRefusal> {
        Self::prepare_fields(weights, weight_bytes, scales, scale_bytes, None, packing)
    }
    fn prepare_fields(
        weights: &'a TensorValue,
        weight_bytes: &'a [u8],
        scales: &'a TensorValue,
        scale_bytes: &'a [u8],
        bias: Option<(&'a TensorValue, &'a [u8])>,
        packing: CompactMatrixPacking,
    ) -> Result<Self, CompactTensorRefusal> {
        Self::prepare_views(
            FixedTensorView::borrowed(weights, weight_bytes),
            FixedTensorView::borrowed(scales, scale_bytes),
            bias.map(|(tensor, bytes)| FixedTensorView::borrowed(tensor, bytes)),
            packing,
        )
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn prepare_owned(
        weights: Arc<AdmittedFixedTensorResource>,
        scales: Arc<AdmittedFixedTensorResource>,
        bias: Option<Arc<AdmittedFixedTensorResource>>,
        packing: CompactMatrixPacking,
    ) -> Result<Self, CompactTensorRefusal> {
        Self::prepare_views(
            FixedTensorView::Owned(weights),
            FixedTensorView::Owned(scales),
            bias.map(FixedTensorView::Owned),
            packing,
        )
    }
    fn prepare_views(
        weights: FixedTensorView<'a>,
        scales: FixedTensorView<'a>,
        bias: Option<FixedTensorView<'a>>,
        packing: CompactMatrixPacking,
    ) -> Result<Self, CompactTensorRefusal> {
        let weight_bytes = weights.bytes();
        let scale_bytes = scales.bytes();
        if INPUT == 0 || OUTPUT == 0 || !INPUT.is_multiple_of(4) || !OUTPUT.is_multiple_of(8) {
            return Err(CompactTensorRefusal::Shape);
        }
        validate(
            weights.tensor(),
            weight_bytes,
            TensorElement::I8,
            &[INPUT as u64, OUTPUT as u64],
        )?;
        validate(
            scales.tensor(),
            scale_bytes,
            TensorElement::F32,
            &[OUTPUT as u64],
        )?;
        if let Some(view) = bias.as_ref() {
            validate(
                view.tensor(),
                view.bytes(),
                TensorElement::F32,
                &[OUTPUT as u64],
            )?;
        }
        for o in 0..OUTPUT {
            let scale = f32_at(scale_bytes, o);
            if !scale.is_finite() || scale <= 0.0 {
                return Err(CompactTensorRefusal::Scale);
            }
            if bias
                .as_ref()
                .is_some_and(|view| !f32_at(view.bytes(), o).is_finite())
            {
                return Err(CompactTensorRefusal::Nonfinite);
            }
        }
        // Each integer product is exact; admitted dimension prevents overflow
        // of the explicitly selected signed I32 accumulation profile.
        if INPUT > (i32::MAX as usize / (128 * 127)) {
            return Err(CompactTensorRefusal::Shape);
        }
        Ok(Self {
            weights,
            scales,
            bias,
            packing,
        })
    }
    pub fn resources(&self) -> [Option<&TensorValue>; 3] {
        [
            Some(self.weights.tensor()),
            Some(self.scales.tensor()),
            self.bias.as_ref().map(FixedTensorView::tensor),
        ]
    }
    pub fn packing(&self) -> CompactMatrixPacking {
        self.packing
    }
    pub fn evaluate(
        &self,
        input: &[i8; INPUT],
        output: &mut [f32; OUTPUT],
    ) -> Result<(), CompactTensorRefusal> {
        if input.contains(&i8::MIN) {
            return Err(CompactTensorRefusal::InputDomain);
        }
        let mut staged = [0.0; OUTPUT];
        for (o, value) in staged.iter_mut().enumerate() {
            let mut sum = 0i32;
            for (i, x) in input.iter().enumerate() {
                let offset = ((o / 8) * (INPUT / 4) + i / 4) * 32 + (o % 8) * 4 + i % 4;
                let w = self.weights.bytes()[offset] as i8;
                sum = sum
                    .checked_add(i32::from(w) * i32::from(*x))
                    .ok_or(CompactTensorRefusal::Overflow)?;
            }
            // Scale contains the selected input/weight quantization conversion;
            // no hidden 1/127, bias substitution, or unsigned-input correction.
            *value = sum as f32 * f32_at(self.scales.bytes(), o);
            if let Some(view) = self.bias.as_ref() {
                *value += f32_at(view.bytes(), o);
            }
            if !value.is_finite() {
                return Err(CompactTensorRefusal::Nonfinite);
            }
        }
        *output = staged;
        Ok(())
    }
}
