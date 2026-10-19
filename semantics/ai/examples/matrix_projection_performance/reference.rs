//! Prior scalar traversal from36c6ff524, over the same already validated bytes.
//! Omits one view-field lookup; this conservatively omits reference bookkeeping.
use conduit_ai::{fixed_neural::FixedNumericRefusal, fixed_tensor::FixedMatrixOrder};
#[inline(never)]
pub(super) fn apply<const I: usize, const O: usize>(
    bytes: &[u8],
    input: &[f32; I],
    output: &mut [f32; O],
    order: FixedMatrixOrder,
) -> Result<(), FixedNumericRefusal> {
    if input.iter().any(|v| !v.is_finite()) {
        return Err(FixedNumericRefusal::NonfiniteInput);
    }
    let mut staged = [0.; O];
    for (row, result) in staged.iter_mut().enumerate() {
        let mut sum = 0.0_f32;
        for (column, value) in input.iter().enumerate() {
            let index = match order {
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
