//! Coordinate conversion shared by native pointer presentation paths.
pub(crate) fn normalized_local(value: u16, extent: u32) -> Result<i64, &'static str> {
    if extent == 0 || u32::from(value) >= extent {
        return Err("compositor-pointer-local-coordinate-invalid");
    }
    // Round upward so the semantic decoder's floor maps back to this exact
    // pixel, including the first pixel on a card edge.
    i64::try_from((u64::from(value) * 1_000_000).div_ceil(u64::from(extent)))
        .map_err(|_| "compositor-pointer-local-coordinate-invalid")
}

pub(crate) fn display_coordinate(value: i64, extent: u32) -> Result<u32, &'static str> {
    if !(0..=1_000_000).contains(&value) || extent == 0 {
        return Err("compositor-pointer-coordinate-invalid");
    }
    let coordinate = u64::try_from(value)
        .ok()
        .and_then(|value| value.checked_mul(u64::from(extent)))
        .map(|value| value / 1_000_000)
        .ok_or("compositor-pointer-coordinate-invalid")?;
    u32::try_from(coordinate.min(u64::from(extent - 1)))
        .map_err(|_| "compositor-pointer-coordinate-invalid")
}
