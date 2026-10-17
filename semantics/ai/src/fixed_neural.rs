//! Scalar reference numeric kernels for fixed, admitted neural shapes.
//! These helpers contain no graph, layer ordering, model identity or recurrence
//! policy. A selected graph/Back must supply those independently.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedNumericRefusal {
    EmptyShape,
    NonfiniteWeight,
    NonfiniteBias,
    NonfiniteInput,
    NonfiniteOutput,
    InvalidScale,
    Index,
}

/// Row-major weights: output[row] = bias[row] + sum(weight[row][col]*input[col]).
/// Dimensions are compile-time fixed; preparation borrows immutable weights.
pub struct FixedAffine<'a, const INPUT: usize, const OUTPUT: usize> {
    weights: &'a [[f32; INPUT]; OUTPUT],
    bias: &'a [f32; OUTPUT],
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedAffine<'a, INPUT, OUTPUT> {
    pub fn prepare(
        weights: &'a [[f32; INPUT]; OUTPUT],
        bias: &'a [f32; OUTPUT],
    ) -> Result<Self, FixedNumericRefusal> {
        if INPUT == 0 || OUTPUT == 0 {
            return Err(FixedNumericRefusal::EmptyShape);
        }
        if weights.iter().flatten().any(|value| !value.is_finite()) {
            return Err(FixedNumericRefusal::NonfiniteWeight);
        }
        if bias.iter().any(|value| !value.is_finite()) {
            return Err(FixedNumericRefusal::NonfiniteBias);
        }
        Ok(Self { weights, bias })
    }

    /// Scalar f32 multiply then add in ascending column order; no fused multiply
    /// add. Output changes only after the complete result is finite.
    pub fn apply(
        &self,
        input: &[f32; INPUT],
        output: &mut [f32; OUTPUT],
    ) -> Result<(), FixedNumericRefusal> {
        finite_input(input)?;
        let mut staged = [0.0; OUTPUT];
        for (row, value) in staged.iter_mut().enumerate() {
            let mut sum = self.bias[row];
            for (weight, input) in self.weights[row].iter().zip(input) {
                sum += weight * input;
            }
            if !sum.is_finite() {
                return Err(FixedNumericRefusal::NonfiniteOutput);
            }
            *value = sum;
        }
        *output = staged;
        Ok(())
    }
}

/// Explicit per-output symmetric signed-eight-bit weight profile. No claim of
/// compatibility with a particular upstream packing/quantization scheme.
pub struct FixedAffineI8<'a, const INPUT: usize, const OUTPUT: usize> {
    weights: &'a [[i8; INPUT]; OUTPUT],
    scales: &'a [f32; OUTPUT],
    bias: &'a [f32; OUTPUT],
}
impl<'a, const INPUT: usize, const OUTPUT: usize> FixedAffineI8<'a, INPUT, OUTPUT> {
    pub fn prepare(
        weights: &'a [[i8; INPUT]; OUTPUT],
        scales: &'a [f32; OUTPUT],
        bias: &'a [f32; OUTPUT],
    ) -> Result<Self, FixedNumericRefusal> {
        if INPUT == 0 || OUTPUT == 0 {
            return Err(FixedNumericRefusal::EmptyShape);
        }
        if scales
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(FixedNumericRefusal::InvalidScale);
        }
        if bias.iter().any(|value| !value.is_finite()) {
            return Err(FixedNumericRefusal::NonfiniteBias);
        }
        Ok(Self {
            weights,
            scales,
            bias,
        })
    }
    pub fn apply(
        &self,
        input: &[f32; INPUT],
        output: &mut [f32; OUTPUT],
    ) -> Result<(), FixedNumericRefusal> {
        finite_input(input)?;
        let mut staged = [0.0; OUTPUT];
        for (row, value) in staged.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (weight, input) in self.weights[row].iter().zip(input) {
                sum += f32::from(*weight) * input;
            }
            let result = self.bias[row] + self.scales[row] * sum;
            if !result.is_finite() {
                return Err(FixedNumericRefusal::NonfiniteOutput);
            }
            *value = result;
        }
        *output = staged;
        Ok(())
    }
}

pub fn fixed_embedding<const ROWS: usize, const WIDTH: usize>(
    table: &[[f32; WIDTH]; ROWS],
    index: usize,
    output: &mut [f32; WIDTH],
) -> Result<(), FixedNumericRefusal> {
    if ROWS == 0 || WIDTH == 0 {
        return Err(FixedNumericRefusal::EmptyShape);
    }
    let row = table.get(index).ok_or(FixedNumericRefusal::Index)?;
    if row.iter().any(|value| !value.is_finite()) {
        return Err(FixedNumericRefusal::NonfiniteWeight);
    }
    *output = *row;
    Ok(())
}

pub fn fixed_tanh<const SIZE: usize>(
    input: &[f32; SIZE],
    output: &mut [f32; SIZE],
) -> Result<(), FixedNumericRefusal> {
    map_activation(input, output, libm::tanhf)
}

pub fn fixed_sigmoid<const SIZE: usize>(
    input: &[f32; SIZE],
    output: &mut [f32; SIZE],
) -> Result<(), FixedNumericRefusal> {
    map_activation(input, output, |value| {
        // Avoid exp overflow for large negative inputs.
        if value >= 0.0 {
            1.0 / (1.0 + libm::expf(-value))
        } else {
            let exponential = libm::expf(value);
            exponential / (1.0 + exponential)
        }
    })
}

fn finite_input<const SIZE: usize>(input: &[f32; SIZE]) -> Result<(), FixedNumericRefusal> {
    if SIZE == 0 {
        return Err(FixedNumericRefusal::EmptyShape);
    }
    if input.iter().any(|value| !value.is_finite()) {
        return Err(FixedNumericRefusal::NonfiniteInput);
    }
    Ok(())
}
fn map_activation<const SIZE: usize>(
    input: &[f32; SIZE],
    output: &mut [f32; SIZE],
    operation: impl Fn(f32) -> f32,
) -> Result<(), FixedNumericRefusal> {
    finite_input(input)?;
    let mut staged = [0.0; SIZE];
    for (value, input) in staged.iter_mut().zip(input) {
        *value = operation(*input);
        if !value.is_finite() {
            return Err(FixedNumericRefusal::NonfiniteOutput);
        }
    }
    *output = staged;
    Ok(())
}
