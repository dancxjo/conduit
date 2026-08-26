use crate::canvas::SoftwareCanvas;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::{Drawable, Point, Primitive, RgbColor};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

#[test]
fn embedded_graphics_primitives_clip_to_the_finite_surface() {
    let guard = 0x00AA_55AA;
    let mut storage = [guard; 4 * 4 + 2];
    {
        let mut canvas = SoftwareCanvas::new(&mut storage[1..17], 4, 4);
        Rectangle::new(
            Point::new(-2, -2),
            embedded_graphics::geometry::Size::new(5, 5),
        )
        .into_styled(PrimitiveStyle::with_fill(Rgb888::new(1, 2, 3)))
        .draw(&mut canvas)
        .unwrap();
    }
    assert_eq!(storage[0], guard);
    assert_eq!(storage[17], guard);
    assert_eq!(storage[1], 0x0001_0203);
    assert_eq!(storage[1 + 2 * 4 + 2], 0x0001_0203);
    assert_eq!(storage[1 + 3 * 4 + 3], guard);
}

#[test]
fn inconsistent_surface_dimensions_cannot_index_past_the_slice() {
    let mut storage = [0; 1];
    let mut canvas = SoftwareCanvas::new(&mut storage, usize::MAX, usize::MAX);
    Rectangle::new(
        Point::new(100, 100),
        embedded_graphics::geometry::Size::new(2, 2),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb888::WHITE))
    .draw(&mut canvas)
    .unwrap();
    assert_eq!(storage, [0]);
}
