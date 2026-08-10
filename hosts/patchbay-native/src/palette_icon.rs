//! Native consumption of the canonical palette icon identity and generated masks.

use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Pixel, Point},
};
use patchbay_model::PaletteIconKey;

use crate::{
    icon::{draw_icon, Icon},
    palette_icon_data,
};

/// Draws the canonical mask and returns `true` only for the detectable generic fallback.
pub(super) fn draw_palette_icon<D>(
    target: &mut D,
    key: PaletteIconKey,
    origin: Point,
    color: Rgb888,
) -> bool
where
    D: DrawTarget<Color = Rgb888>,
{
    let Some(mask) = palette_icon_data::mask(key) else {
        draw_icon(target, Icon::Gear, origin, color);
        return true;
    };
    let pixels = mask.iter().enumerate().flat_map(|(y, row)| {
        (0..16).filter_map(move |x| {
            (row & (1 << (15 - x)) != 0).then_some(Pixel(origin + Point::new(x, y as i32), color))
        })
    });
    let _ = target.draw_iter(pixels);
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::SoftwareCanvas;
    use embedded_graphics::pixelcolor::RgbColor;

    #[test]
    fn every_canonical_upstream_key_has_a_nonempty_native_mask() {
        for key in PaletteIconKey::ALL_UPSTREAM {
            let mask = palette_icon_data::mask(key).expect("generated native mask");
            assert!(
                mask.iter().any(|row| *row != 0),
                "empty {} mask",
                key.as_str()
            );
        }
    }

    #[test]
    fn missing_icon_uses_the_detectable_generic_fallback() {
        let mut pixels = [0_u32; 16 * 16];
        let mut canvas = SoftwareCanvas::new(&mut pixels, 16, 16);
        assert!(draw_palette_icon(
            &mut canvas,
            PaletteIconKey::GenericGear,
            Point::zero(),
            Rgb888::WHITE,
        ));
        assert!(pixels.iter().any(|pixel| *pixel != 0));
    }
}
