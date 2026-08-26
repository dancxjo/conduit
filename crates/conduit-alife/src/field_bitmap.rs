//! Exact semantic lowering from a Lenia scalar field to portable gray8 pixels.

use alloc::vec::Vec;
use conduit_core::{LeniaFieldState, LENIA_Q16_ONE};
use conduit_presentation::{BitmapRefusal, Gray8Bitmap};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldBitmapRefusal {
    InvalidField,
    InvalidBitmap,
}

pub fn lenia_field_to_gray8(field: &LeniaFieldState) -> Result<Gray8Bitmap, FieldBitmapRefusal> {
    field
        .validate()
        .map_err(|_| FieldBitmapRefusal::InvalidField)?;
    let pixels = field
        .cells()
        .iter()
        .map(|value| {
            ((u64::from(*value) * 255 + u64::from(LENIA_Q16_ONE / 2)) / u64::from(LENIA_Q16_ONE))
                as u8
        })
        .collect::<Vec<_>>();
    Gray8Bitmap::new(field.width, field.height, pixels).map_err(map_bitmap_refusal)
}

fn map_bitmap_refusal(_: BitmapRefusal) -> FieldBitmapRefusal {
    FieldBitmapRefusal::InvalidBitmap
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::LeniaFieldId;

    #[test]
    fn q16_field_maps_deterministically_without_presentation_facts() {
        let field = LeniaFieldState::from_cells(
            LeniaFieldId([7; 16]),
            3,
            32,
            32,
            (0..1024)
                .map(|index| match index % 4 {
                    0 => 0,
                    1 => LENIA_Q16_ONE / 4,
                    2 => LENIA_Q16_ONE / 2,
                    _ => LENIA_Q16_ONE,
                })
                .collect(),
        )
        .unwrap();
        let bitmap = lenia_field_to_gray8(&field).unwrap();
        assert_eq!((bitmap.width(), bitmap.height()), (32, 32));
        assert_eq!(&bitmap.pixels()[..4], &[0, 64, 128, 255]);
        assert_eq!(
            conduit_presentation::Gray8Bitmap::decode(&bitmap.encode()),
            Ok(bitmap)
        );
    }
}
