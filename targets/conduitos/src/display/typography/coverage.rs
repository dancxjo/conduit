//! Fixed four-tap coverage reconstruction at destination pixel centers.
//! Clamp to the admitted glyph bounds; no atlas neighbor or new ink extent.
pub(super) fn sample(
    x: u8,
    y: u8,
    width: u8,
    height: u8,
    percent: u16,
    coverage: impl Fn(u8, u8) -> u8,
) -> u8 {
    if width == 0 || height == 0 {
        return 0;
    }
    let axis = |pixel: u8, size: u8| {
        let coordinate = ((i32::from(pixel) * 2 + 1) * 100 * 256) / (i32::from(percent) * 2) - 128;
        let base = coordinate.div_euclid(256);
        let fraction = coordinate.rem_euclid(256) as u32;
        let last = i32::from(size) - 1;
        (
            base.clamp(0, last) as u8,
            (base + 1).clamp(0, last) as u8,
            fraction,
        )
    };
    let (x0, x1, wx) = axis(x, width);
    let (y0, y1, wy) = axis(y, height);
    let top = u32::from(coverage(x0, y0)) * (256 - wx) + u32::from(coverage(x1, y0)) * wx;
    let bottom = u32::from(coverage(x0, y1)) * (256 - wx) + u32::from(coverage(x1, y1)) * wx;
    ((top * (256 - wy) + bottom * wy + 32_768) >> 16) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centers_interpolate_and_edges_never_sample_another_glyph() {
        let source = [[0, 0], [0, 255]];
        let read = |x: u8, y: u8| source[usize::from(y)][usize::from(x)];
        assert_eq!(sample(0, 0, 2, 2, 200, read), 0);
        assert_eq!(sample(1, 1, 2, 2, 200, read), 16);
        assert_eq!(sample(2, 2, 2, 2, 200, read), 143);
        assert_eq!(sample(3, 3, 2, 2, 200, read), 255);
        for percent in [100, 125, 150, 200] {
            for y in 0..4 {
                for x in 0..4 {
                    assert_eq!(sample(x, y, 2, 2, percent, |_, _| 88), 88);
                }
            }
        }
    }
}
