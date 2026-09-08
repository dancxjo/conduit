//! Finite presenter-local scroll state for independent native surfaces.

use conduit_presentation::Presentation;

use crate::{display::PixelTarget, native_compositor::FrameReceipt};

use super::{
    CompositionReceipt, RoutedPointer, Slot, TourShellError, TourShellPresenter, inspector_scene,
    transient_scene,
};

pub const MAX_SCROLL_CONTENT_HEIGHT: u16 = 768;
const SCROLL_STEP: u16 = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollDirection {
    Start,
    Backward,
    Forward,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollState {
    viewport_height: u16,
    content_height: u16,
    offset: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShellScrollReceipt {
    pub surface_id: alloc::string::String,
    pub previous_offset: u16,
    pub current_offset: u16,
    pub composition: CompositionReceipt,
    pub frame: FrameReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
// The updated receipt stays inline so scrolling never introduces heap allocation.
#[allow(clippy::large_enum_variant)]
pub enum ScrollOutcome {
    Updated(ShellScrollReceipt),
    Boundary,
    Ineligible,
}

impl ScrollState {
    pub const fn empty() -> Self {
        Self {
            viewport_height: 0,
            content_height: 0,
            offset: 0,
        }
    }

    pub fn configure(
        &mut self,
        viewport_height: u16,
        content_height: u16,
    ) -> Result<(), TourShellError> {
        if viewport_height == 0 || content_height > MAX_SCROLL_CONTENT_HEIGHT {
            return Err(TourShellError::Identity);
        }
        self.viewport_height = viewport_height;
        self.content_height = content_height;
        self.offset = self.offset.min(self.maximum_offset());
        Ok(())
    }

    pub const fn offset(self) -> u16 {
        self.offset
    }
    pub const fn maximum_offset(self) -> u16 {
        self.content_height.saturating_sub(self.viewport_height)
    }

    pub fn apply(&mut self, direction: ScrollDirection) -> bool {
        let next = match direction {
            ScrollDirection::Start => 0,
            ScrollDirection::Backward => self.offset.saturating_sub(SCROLL_STEP),
            ScrollDirection::Forward => self
                .offset
                .saturating_add(SCROLL_STEP)
                .min(self.maximum_offset()),
            ScrollDirection::End => self.maximum_offset(),
        };
        let changed = next != self.offset;
        self.offset = next;
        changed
    }

    pub fn content_y(self, viewport_y: u16) -> Result<u16, TourShellError> {
        if viewport_y >= self.viewport_height {
            return Err(TourShellError::Identity);
        }
        viewport_y
            .checked_add(self.offset)
            .ok_or(TourShellError::Identity)
    }
}

impl TourShellPresenter {
    pub fn scroll_focused(
        &mut self,
        direction: ScrollDirection,
        display: &mut impl PixelTarget,
    ) -> Result<ScrollOutcome, TourShellError> {
        let Some(focused) = self.compositor.focused_surface() else {
            return Ok(ScrollOutcome::Ineligible);
        };
        let Some(index) = self
            .surfaces
            .iter()
            .position(|state| state.slot.surface() == focused)
        else {
            return Ok(ScrollOutcome::Ineligible);
        };
        let slot = self.surfaces[index].slot;
        if !matches!(slot, Slot::Inspector | Slot::Transient) {
            return Ok(ScrollOutcome::Ineligible);
        }
        let previous_offset = self.surfaces[index].scroll.offset();
        if !self.surfaces[index].scroll.apply(direction) {
            return Ok(ScrollOutcome::Boundary);
        }
        let current_offset = self.surfaces[index].scroll.offset();
        let bounds = self.surfaces[index]
            .bounds
            .ok_or(TourShellError::Identity)?;
        let presentation = self.surfaces[index]
            .presentation
            .clone()
            .ok_or(TourShellError::Identity)?;
        let face = self.surfaces[index]
            .face_subject
            .clone()
            .ok_or(TourShellError::Identity)?;
        let revised = revised(presentation)?;
        let scene = match slot {
            Slot::Inspector => inspector_scene(bounds, &revised, current_offset)?,
            Slot::Transient => transient_scene(bounds, &revised, current_offset)?,
            _ => return Ok(ScrollOutcome::Ineligible),
        };
        let composition = self.present_surface(
            slot,
            &revised,
            &face,
            bounds,
            if slot == Slot::Inspector { 2 } else { 3 },
            &scene,
        )?;
        let frame = self
            .compositor
            .compose_frame(display)
            .map_err(TourShellError::Compositor)?;
        Ok(ScrollOutcome::Updated(ShellScrollReceipt {
            surface_id: slot.surface().into(),
            previous_offset,
            current_offset,
            composition,
            frame,
        }))
    }

    pub fn scroll_hit_subject(&self, route: &RoutedPointer) -> Result<Option<u8>, TourShellError> {
        self.validate_pointer_route(route)?;
        let Some(state) = self
            .surfaces
            .iter()
            .find(|state| state.slot.surface() == route.surface_id)
        else {
            return Err(TourShellError::Identity);
        };
        if !matches!(state.slot, Slot::Inspector | Slot::Transient) {
            return Ok(None);
        }
        row_at(state.scroll.content_y(route.local_y)?)
    }
}

fn revised(presentation: Presentation) -> Result<Presentation, TourShellError> {
    Presentation::new(
        presentation
            .revision
            .checked_add(1)
            .ok_or(TourShellError::Identity)?,
        presentation.basis,
        presentation.subjects,
        presentation.relationships,
        presentation.properties,
        presentation.text,
    )
    .map_err(|_| TourShellError::Identity)
}

fn row_at(content_y: u16) -> Result<Option<u8>, TourShellError> {
    if content_y < 76 {
        return Ok(None);
    }
    let row = (content_y - 76) / 140;
    let within = (content_y - 76) % 140;
    Ok(
        (row < 5 && within < 60)
            .then_some(u8::try_from(row).map_err(|_| TourShellError::Identity)?),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_boundaries_and_resize_clamping_are_exact() {
        let mut state = ScrollState::empty();
        state.configure(200, 768).unwrap();
        assert!(state.apply(ScrollDirection::End));
        assert_eq!(state.offset(), 568);
        assert_eq!(state.content_y(10).unwrap(), 578);
        state.configure(700, 768).unwrap();
        assert_eq!(state.offset(), 68);
        assert!(!state.apply(ScrollDirection::End));
    }

    #[test]
    fn empty_overflow_and_outside_viewport_refuse_distinctly() {
        let mut state = ScrollState::empty();
        assert_eq!(state.configure(0, 10), Err(TourShellError::Identity));
        assert_eq!(state.configure(10, 769), Err(TourShellError::Identity));
        state.configure(10, 10).unwrap();
        assert_eq!(state.content_y(10), Err(TourShellError::Identity));
    }

    #[test]
    fn translated_hits_never_admit_row_gaps_or_content_outside_the_viewport() {
        let mut state = ScrollState::empty();
        state.configure(120, 768).unwrap();
        state.apply(ScrollDirection::Forward);
        assert_eq!(row_at(state.content_y(30).unwrap()).unwrap(), Some(0));
        assert_eq!(row_at(state.content_y(90).unwrap()).unwrap(), None);
        assert_eq!(state.content_y(120), Err(TourShellError::Identity));
    }
}
