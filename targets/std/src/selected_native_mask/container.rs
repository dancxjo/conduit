//! Pure container geometry and finite backing-fit checks. This value is neither
//! an enclosing owner's placement authority nor surface/resource possession.
//! The production caller must separately retain those actual owners and recheck
//! their current geometry and reservation before changing a presentation.
use conduit_face::LayoutRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeMaskContainerMode {
    Tile,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeMaskContainerRefusal {
    ZeroExtent,
    NegativeOrigin,
    CoordinateOverflow,
    OutsideOriginalRaster,
    UnsupportedSceneExtent,
    FullscreenMismatch,
    TileNotStrict,
    BackingOverflow,
    Unfunded,
}

/// Named independently computed footprints; none of these numbers grants memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NativeMaskContainerBacking {
    pub scene: usize,
    pub frame: usize,
    pub compositor: usize,
}

/// Immutable geometry observation. Original output dimensions remain separate
/// from the local Scene extent; a tile never redefines or stretches the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CheckedNativeMaskContainer {
    original: (u32, u32),
    rectangle: LayoutRect,
    required_backing: usize,
}
impl CheckedNativeMaskContainer {
    /// Private owner-preparation seam. The caller supplies the current original
    /// raster, an explicitly authorized enclosing rectangle and the remaining
    /// bytes of its actual finite reservation. This pure check cannot establish
    /// that custody from copied dimensions, an enum or a scalar byte count.
    pub(super) fn check(
        original: (u32, u32),
        rectangle: LayoutRect,
        mode: NativeMaskContainerMode,
        backing: NativeMaskContainerBacking,
        available_backing: usize,
    ) -> Result<Self, NativeMaskContainerRefusal> {
        use NativeMaskContainerRefusal as Refusal;
        if original.0 == 0 || original.1 == 0 || rectangle.width == 0 || rectangle.height == 0 {
            return Err(Refusal::ZeroExtent);
        }
        if rectangle.x < 0 || rectangle.y < 0 {
            return Err(Refusal::NegativeOrigin);
        }
        let x = u16::try_from(rectangle.x).map_err(|_| Refusal::CoordinateOverflow)?;
        let y = u16::try_from(rectangle.y).map_err(|_| Refusal::CoordinateOverflow)?;
        let right = x
            .checked_add(rectangle.width)
            .ok_or(Refusal::CoordinateOverflow)?;
        let bottom = y
            .checked_add(rectangle.height)
            .ok_or(Refusal::CoordinateOverflow)?;
        if u32::from(right) > original.0 || u32::from(bottom) > original.1 {
            return Err(Refusal::OutsideOriginalRaster);
        }
        // Exactly the existing native Scene geometry contract, without relaxing
        // its command, glyph, frame or document budgets.
        if rectangle.width < 320
            || rectangle.height < 240
            || rectangle.width > 16_384
            || rectangle.height > 16_384
        {
            return Err(Refusal::UnsupportedSceneExtent);
        }
        let full = rectangle.x == 0
            && rectangle.y == 0
            && u32::from(rectangle.width) == original.0
            && u32::from(rectangle.height) == original.1;
        match mode {
            NativeMaskContainerMode::Fullscreen if !full => {
                return Err(Refusal::FullscreenMismatch)
            }
            NativeMaskContainerMode::Tile if full => return Err(Refusal::TileNotStrict),
            _ => {}
        }
        let required_backing = backing
            .scene
            .checked_add(backing.frame)
            .and_then(|bytes| bytes.checked_add(backing.compositor))
            .ok_or(Refusal::BackingOverflow)?;
        if required_backing > available_backing {
            return Err(Refusal::Unfunded);
        }
        Ok(Self {
            original,
            rectangle,
            required_backing,
        })
    }

    pub(super) fn original_raster_extent(&self) -> (u32, u32) {
        self.original
    }
    pub(super) fn rectangle(&self) -> LayoutRect {
        self.rectangle
    }
    pub(super) fn local_scene_extent(&self) -> (u16, u16) {
        (self.rectangle.width, self.rectangle.height)
    }
    pub(super) fn required_backing_bytes(&self) -> usize {
        self.required_backing
    }
}

#[cfg(test)]
#[path = "container_tests.rs"]
mod tests;
