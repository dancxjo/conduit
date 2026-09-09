//! Bounded compositor-owned cursor and surface-focus affordances.

use super::{NativeCompositor, NativeCompositorError, damage::RawDamageRect};

const CURSOR_WIDTH: u16 = 10;
const CURSOR_HEIGHT: u16 = 16;

impl NativeCompositor {
    pub fn set_cursor_hover(&mut self, hovered: bool) -> Result<(), NativeCompositorError> {
        if self.cursor_hover == hovered {
            return Ok(());
        }
        if let Some((x, y)) = self.cursor {
            self.damage.add(RawDamageRect::new(
                i32::try_from(x).map_err(|_| NativeCompositorError::InvalidBounds)?,
                i32::try_from(y).map_err(|_| NativeCompositorError::InvalidBounds)?,
                CURSOR_WIDTH,
                CURSOR_HEIGHT,
            )?);
        }
        self.cursor_hover = hovered;
        Ok(())
    }
    pub(super) fn move_cursor(&mut self, x: u32, y: u32) -> Result<(), NativeCompositorError> {
        if self.cursor == Some((x, y)) {
            return Ok(());
        }
        if let Some((old_x, old_y)) = self.cursor {
            self.damage.add(RawDamageRect::new(
                i32::try_from(old_x).map_err(|_| NativeCompositorError::InvalidBounds)?,
                i32::try_from(old_y).map_err(|_| NativeCompositorError::InvalidBounds)?,
                CURSOR_WIDTH,
                CURSOR_HEIGHT,
            )?);
        }
        self.damage.add(RawDamageRect::new(
            i32::try_from(x).map_err(|_| NativeCompositorError::InvalidBounds)?,
            i32::try_from(y).map_err(|_| NativeCompositorError::InvalidBounds)?,
            CURSOR_WIDTH,
            CURSOR_HEIGHT,
        )?);
        self.cursor = Some((x, y));
        Ok(())
    }

    pub(super) fn damage_focus(&mut self, surface_id: &str) -> Result<(), NativeCompositorError> {
        if let Some(surface) = self
            .surfaces
            .iter()
            .find(|surface| surface.surface_id == surface_id)
        {
            self.damage.add_layout(surface.bounds)?;
        }
        Ok(())
    }
}
