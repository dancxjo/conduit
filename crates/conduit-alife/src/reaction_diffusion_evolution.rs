//! Exact host-neutral synchronous update arithmetic for the reviewed Gray-Scott profile.

use crate::{GrayScottParameters, ReactionDiffusionCell, ReactionDiffusionRefusal};

const CONCENTRATION_SCALE: i64 = 1_000_000;

pub(crate) fn evolve_generation(
    current: &[ReactionDiffusionCell],
    next: &mut [ReactionDiffusionCell],
    width: usize,
    height: usize,
    parameters: GrayScottParameters,
) -> Result<(), ReactionDiffusionRefusal> {
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            next[index] = evolve_cell(
                current[index],
                current[((y + height - 1) % height) * width + x],
                current[((y + 1) % height) * width + x],
                current[y * width + ((x + width - 1) % width)],
                current[y * width + ((x + 1) % width)],
                parameters,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn evolve_cell(
    center: ReactionDiffusionCell,
    north: ReactionDiffusionCell,
    south: ReactionDiffusionCell,
    west: ReactionDiffusionCell,
    east: ReactionDiffusionCell,
    parameters: GrayScottParameters,
) -> Result<ReactionDiffusionCell, ReactionDiffusionRefusal> {
    let lap_u = laplacian(
        center.u_ppm,
        north.u_ppm,
        south.u_ppm,
        west.u_ppm,
        east.u_ppm,
    );
    let lap_v = laplacian(
        center.v_ppm,
        north.v_ppm,
        south.v_ppm,
        west.v_ppm,
        east.v_ppm,
    );
    let reaction = scaled_product(
        scaled_product(i64::from(center.u_ppm), i64::from(center.v_ppm))?,
        i64::from(center.v_ppm),
    )?;
    let feed = scaled_product(
        i64::from(parameters.feed_ppm),
        CONCENTRATION_SCALE - i64::from(center.u_ppm),
    )?;
    let removal = scaled_product(
        i64::from(parameters.feed_ppm + parameters.kill_ppm),
        i64::from(center.v_ppm),
    )?;
    let delta_u = scaled_product(
        i64::from(parameters.time_step_ppm),
        scaled_product(i64::from(parameters.diffusion_u_ppm), lap_u)? - reaction + feed,
    )?;
    let delta_v = scaled_product(
        i64::from(parameters.time_step_ppm),
        scaled_product(i64::from(parameters.diffusion_v_ppm), lap_v)? + reaction - removal,
    )?;
    Ok(ReactionDiffusionCell {
        u_ppm: clamp_concentration(i64::from(center.u_ppm) + delta_u),
        v_ppm: clamp_concentration(i64::from(center.v_ppm) + delta_v),
    })
}

fn laplacian(center: u32, north: u32, south: u32, west: u32, east: u32) -> i64 {
    i64::from(north) + i64::from(south) + i64::from(west) + i64::from(east) - 4 * i64::from(center)
}

fn scaled_product(left: i64, right: i64) -> Result<i64, ReactionDiffusionRefusal> {
    let product = i128::from(left) * i128::from(right) / i128::from(CONCENTRATION_SCALE);
    i64::try_from(product).map_err(|_| ReactionDiffusionRefusal::ArithmeticOverflow)
}

fn clamp_concentration(value: i64) -> u32 {
    value.clamp(0, CONCENTRATION_SCALE) as u32
}
