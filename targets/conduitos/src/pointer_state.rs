//! Shared physical pointer history, independent of Face and Show identity.
use conduit_semantic_catalog::NormalizedPointerSample;

pub(crate) struct PointerState {
    x: i64,
    y: i64,
    primary_pressed: bool,
    sequence: u64,
    retired: bool,
}

impl PointerState {
    pub(crate) const fn new() -> Self {
        Self {
            x: 500_000,
            y: 500_000,
            primary_pressed: false,
            sequence: 0,
            retired: false,
        }
    }

    pub(crate) fn is_retired(&self) -> bool {
        self.retired
    }

    pub(crate) fn retire(&mut self) {
        self.retired = true;
    }

    pub(crate) fn observe(
        &mut self,
        sample: NormalizedPointerSample,
    ) -> Result<bool, &'static str> {
        if self.retired {
            return Err("product-pointer-retired");
        }
        if !(0..=1_000_000).contains(&sample.position_x)
            || !(0..=1_000_000).contains(&sample.position_y)
            || sample.sequence <= self.sequence
        {
            return Err("product-pointer-sample-invalid");
        }
        // A coalesced button history cannot establish a unique press edge.
        if sample.coalesced != 0 || sample.dropped != 0 {
            return Err("product-pointer-gesture-history-gap");
        }
        let activate = sample.primary_pressed && !self.primary_pressed;
        self.x = sample.position_x;
        self.y = sample.position_y;
        self.primary_pressed = sample.primary_pressed;
        self.sequence = sample.sequence;
        Ok(activate)
    }

    pub(crate) fn position(
        &self,
        display: &impl crate::display::PixelTarget,
    ) -> Result<(u32, u32), &'static str> {
        let format = display
            .format()
            .validate()
            .map_err(crate::display::DisplayError::as_str)?;
        Ok((
            crate::pointer_coordinates::display_coordinate(self.x, format.width)?,
            crate::pointer_coordinates::display_coordinate(self.y, format.height)?,
        ))
    }
}

#[cfg(test)]
#[path = "pointer_state_tests.rs"]
mod tests;
