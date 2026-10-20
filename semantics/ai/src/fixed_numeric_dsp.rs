//! Finite scalar DSP kernels. Windowing, band layout and feature policy belong to Source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedDspRefusal {
    Shape,
    Nonfinite,
    Nonpositive,
    Negative,
}
fn finite(values: &[f32]) -> Result<(), FixedDspRefusal> {
    if values.iter().all(|x| x.is_finite()) {
        Ok(())
    } else {
        Err(FixedDspRefusal::Nonfinite)
    }
}
/// Unnormalized forward real DFT, interleaved real/imaginary bins0..=N/2.
/// Uses f64 accumulation and libm trigonometry; every output must remain finite f32.
pub fn real_dft<const N: usize, const OUT: usize>(
    input: &[f32; N],
    output: &mut [f32; OUT],
) -> Result<(), FixedDspRefusal> {
    if N < 2 || !N.is_multiple_of(2) || OUT != N + 2 || OUT > 1024 {
        return Err(FixedDspRefusal::Shape);
    }
    finite(input)?;
    let mut staged = [0.; OUT];
    for k in 0..OUT / 2 {
        let mut real = 0.;
        let mut imag = 0.;
        for (n, &value) in input.iter().enumerate() {
            let angle = -2. * core::f64::consts::PI * (k as f64) * (n as f64) / (N as f64);
            real += f64::from(value) * libm::cos(angle);
            imag += f64::from(value) * libm::sin(angle);
        }
        staged[2 * k] = real as f32;
        staged[2 * k + 1] = imag as f32;
    }
    finite(&staged)?;
    *output = staged;
    Ok(())
}
pub fn magnitude_squared<const IN: usize, const OUT: usize>(
    input: &[f32; IN],
    output: &mut [f32; OUT],
) -> Result<(), FixedDspRefusal> {
    if IN != 2 * OUT || OUT == 0 || IN > 1024 {
        return Err(FixedDspRefusal::Shape);
    }
    finite(input)?;
    let mut staged = [0.; OUT];
    for (i, value) in staged.iter_mut().enumerate() {
        let r = f64::from(input[2 * i]);
        let j = f64::from(input[2 * i + 1]);
        *value = (r * r + j * j) as f32;
    }
    finite(&staged)?;
    *output = staged;
    Ok(())
}
pub fn log10<const N: usize>(
    input: &[f32; N],
    output: &mut [f32; N],
) -> Result<(), FixedDspRefusal> {
    finite(input)?;
    if input.iter().any(|x| *x <= 0.) {
        return Err(FixedDspRefusal::Nonpositive);
    }
    let staged = core::array::from_fn(|i| libm::log10f(input[i]));
    finite(&staged)?;
    *output = staged;
    Ok(())
}
pub fn dot<const N: usize>(left: &[f32; N], right: &[f32; N]) -> Result<f32, FixedDspRefusal> {
    finite(left)?;
    finite(right)?;
    let value = left
        .iter()
        .zip(right)
        .map(|(&l, &r)| f64::from(l) * f64::from(r))
        .sum::<f64>() as f32;
    finite(&[value])?;
    Ok(value)
}
pub fn sqrt(input: f32) -> Result<f32, FixedDspRefusal> {
    finite(&[input])?;
    if input < 0. {
        return Err(FixedDspRefusal::Negative);
    }
    Ok(libm::sqrtf(input))
}
