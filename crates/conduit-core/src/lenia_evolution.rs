//! Allocation-free generation arithmetic for the fixed Q16.16 Lenia profile.

use alloc::vec::Vec;

use crate::{LeniaParameters, LeniaRefusal, LENIA_Q16_ONE};

#[derive(Debug, Copy, Clone)]
pub(crate) struct KernelSample {
    pub(crate) dx: i16,
    pub(crate) dy: i16,
    pub(crate) weight: u32,
}

impl KernelSample {
    pub(crate) const EMPTY: Self = Self {
        dx: 0,
        dy: 0,
        weight: 0,
    };
}

pub(crate) struct RegionDimensions {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) halo: usize,
}

pub(crate) fn build_kernel(
    parameters: LeniaParameters,
    output: &mut Vec<KernelSample>,
) -> Result<u64, LeniaRefusal> {
    output.clear();
    build_kernel_with(parameters, |sample| {
        output.push(sample);
        Ok(())
    })
}

pub(crate) fn build_kernel_into(
    parameters: LeniaParameters,
    output: &mut [KernelSample],
) -> Result<(usize, u64), LeniaRefusal> {
    let mut length = 0;
    let total = build_kernel_with(parameters, |sample| {
        let destination = output
            .get_mut(length)
            .ok_or(LeniaRefusal::CellCountMismatch)?;
        *destination = sample;
        length += 1;
        Ok(())
    })?;
    Ok((length, total))
}

fn build_kernel_with(
    parameters: LeniaParameters,
    mut push: impl FnMut(KernelSample) -> Result<(), LeniaRefusal>,
) -> Result<u64, LeniaRefusal> {
    let radius = i32::from(parameters.kernel_radius);
    let radius_squared = radius * radius;
    let mut total = 0_u64;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let distance_squared = dx * dx + dy * dy;
            if distance_squared > radius_squared {
                continue;
            }
            let distance_q16 = integer_sqrt((distance_squared as u128) << 32)?
                / u128::from(parameters.kernel_radius);
            let distance_q16 =
                u32::try_from(distance_q16).map_err(|_| LeniaRefusal::ArithmeticOverflow)?;
            let difference = distance_q16.abs_diff(parameters.kernel_mu_q16);
            let exponent = gaussian_exponent(difference, parameters.kernel_sigma_q16)?;
            let weight = exp_negative_q16(exponent);
            if weight != 0 {
                push(KernelSample {
                    dx: dx as i16,
                    dy: dy as i16,
                    weight,
                })?;
                total = total
                    .checked_add(u64::from(weight))
                    .ok_or(LeniaRefusal::ArithmeticOverflow)?;
            }
        }
    }
    if total == 0 {
        return Err(LeniaRefusal::InvalidParameters);
    }
    Ok(total)
}

pub(crate) fn evolve_generation(
    current: &[u32],
    next: &mut [u32],
    width: usize,
    height: usize,
    parameters: LeniaParameters,
    kernel: &[KernelSample],
    kernel_weight: u64,
) -> Result<(), LeniaRefusal> {
    if current.len() != width * height || next.len() != current.len() || kernel_weight == 0 {
        return Err(LeniaRefusal::CellCountMismatch);
    }
    for y in 0..height {
        for x in 0..width {
            let mut weighted = 0_u128;
            for sample in kernel {
                let source_x = wrapped(x, i32::from(sample.dx), width);
                let source_y = wrapped(y, i32::from(sample.dy), height);
                weighted = weighted
                    .checked_add(
                        u128::from(sample.weight)
                            .checked_mul(u128::from(current[source_y * width + source_x]))
                            .ok_or(LeniaRefusal::ArithmeticOverflow)?,
                    )
                    .ok_or(LeniaRefusal::ArithmeticOverflow)?;
            }
            next[y * width + x] =
                evolve_cell(current[y * width + x], weighted, parameters, kernel_weight)?;
        }
    }
    Ok(())
}

pub(crate) fn evolve_region_generation(
    expanded: &[u32],
    next: &mut [u32],
    dimensions: RegionDimensions,
    parameters: LeniaParameters,
    kernel: &[KernelSample],
    kernel_weight: u64,
) -> Result<(), LeniaRefusal> {
    let RegionDimensions {
        width: region_width,
        height: region_height,
        halo,
    } = dimensions;
    let expanded_width = region_width
        .checked_add(halo * 2)
        .ok_or(LeniaRefusal::CellCountMismatch)?;
    let expanded_height = region_height
        .checked_add(halo * 2)
        .ok_or(LeniaRefusal::CellCountMismatch)?;
    if expanded.len() != expanded_width * expanded_height
        || next.len() != region_width * region_height
        || halo != usize::from(parameters.kernel_radius)
        || kernel_weight == 0
    {
        return Err(LeniaRefusal::CellCountMismatch);
    }
    for y in 0..region_height {
        for x in 0..region_width {
            let center_x = x + halo;
            let center_y = y + halo;
            let mut weighted = 0_u128;
            for sample in kernel {
                let source_x = usize::try_from(center_x as isize + sample.dx as isize)
                    .map_err(|_| LeniaRefusal::CellCountMismatch)?;
                let source_y = usize::try_from(center_y as isize + sample.dy as isize)
                    .map_err(|_| LeniaRefusal::CellCountMismatch)?;
                weighted = weighted
                    .checked_add(
                        u128::from(sample.weight)
                            .checked_mul(u128::from(expanded[source_y * expanded_width + source_x]))
                            .ok_or(LeniaRefusal::ArithmeticOverflow)?,
                    )
                    .ok_or(LeniaRefusal::ArithmeticOverflow)?;
            }
            next[y * region_width + x] = evolve_cell(
                expanded[center_y * expanded_width + center_x],
                weighted,
                parameters,
                kernel_weight,
            )?;
        }
    }
    Ok(())
}

fn evolve_cell(
    current: u32,
    weighted: u128,
    parameters: LeniaParameters,
    kernel_weight: u64,
) -> Result<u32, LeniaRefusal> {
    let potential =
        u32::try_from((weighted + u128::from(kernel_weight / 2)) / u128::from(kernel_weight))
            .map_err(|_| LeniaRefusal::ArithmeticOverflow)?;
    let difference = potential.abs_diff(parameters.growth_mu_q16);
    let exponent = gaussian_exponent(difference, parameters.growth_sigma_q16)?;
    let bell = i64::from(exp_negative_q16(exponent));
    let growth = bell * 2 - i64::from(LENIA_Q16_ONE);
    let delta = multiply_signed_q16(i64::from(parameters.dt_q16), growth)?;
    let value = i64::from(current)
        .checked_add(delta)
        .ok_or(LeniaRefusal::ArithmeticOverflow)?;
    Ok(value.clamp(0, i64::from(LENIA_Q16_ONE)) as u32)
}

fn gaussian_exponent(difference_q16: u32, sigma_q16: u32) -> Result<u32, LeniaRefusal> {
    let numerator = u128::from(difference_q16)
        .checked_mul(u128::from(difference_q16))
        .and_then(|value| value.checked_mul(u128::from(LENIA_Q16_ONE)))
        .ok_or(LeniaRefusal::ArithmeticOverflow)?;
    let denominator = u128::from(sigma_q16)
        .checked_mul(u128::from(sigma_q16))
        .and_then(|value| value.checked_mul(2))
        .ok_or(LeniaRefusal::ArithmeticOverflow)?;
    if denominator == 0 {
        return Err(LeniaRefusal::InvalidParameters);
    }
    u32::try_from((numerator + denominator / 2) / denominator)
        .map_err(|_| LeniaRefusal::ArithmeticOverflow)
}

/// Deterministic integer approximation `(1 + x/256)^-256` of `exp(-x)`.
fn exp_negative_q16(exponent_q16: u32) -> u32 {
    const CUTOFF: u32 = 16 * LENIA_Q16_ONE;
    if exponent_q16 >= CUTOFF {
        return 0;
    }
    let denominator = u64::from(LENIA_Q16_ONE) + u64::from(exponent_q16).div_ceil(256);
    let mut value =
        ((u64::from(LENIA_Q16_ONE) * u64::from(LENIA_Q16_ONE)) + denominator / 2) / denominator;
    for _ in 0..8 {
        value = (value * value + u64::from(LENIA_Q16_ONE / 2)) / u64::from(LENIA_Q16_ONE);
    }
    value.min(u64::from(LENIA_Q16_ONE)) as u32
}

fn multiply_signed_q16(left: i64, right: i64) -> Result<i64, LeniaRefusal> {
    let product = i128::from(left)
        .checked_mul(i128::from(right))
        .ok_or(LeniaRefusal::ArithmeticOverflow)?;
    let rounded = if product >= 0 {
        product + i128::from(LENIA_Q16_ONE / 2)
    } else {
        product - i128::from(LENIA_Q16_ONE / 2)
    };
    i64::try_from(rounded / i128::from(LENIA_Q16_ONE)).map_err(|_| LeniaRefusal::ArithmeticOverflow)
}

fn wrapped(position: usize, delta: i32, extent: usize) -> usize {
    (position as i32 + delta).rem_euclid(extent as i32) as usize
}

fn integer_sqrt(value: u128) -> Result<u128, LeniaRefusal> {
    if value == 0 {
        return Ok(0);
    }
    let mut x = value;
    let mut next = x.div_ceil(2);
    while next < x {
        x = next;
        next = (x + value / x) / 2;
    }
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_exponential_is_bounded_and_monotonic() {
        let mut previous = LENIA_Q16_ONE;
        for input in 0..=16 * LENIA_Q16_ONE {
            let value = exp_negative_q16(input);
            assert!(value <= previous);
            previous = value;
        }
        assert_eq!(exp_negative_q16(0), LENIA_Q16_ONE);
        assert_eq!(exp_negative_q16(16 * LENIA_Q16_ONE), 0);
    }
}
