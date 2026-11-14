//! Local reading controls, never semantic Body actions or interaction payloads.
use super::*;

/// Exact painted and hit-tested footer controls for this immutable document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePageControls {
    pub previous: LayoutRect,
    pub next: LayoutRect,
    pub can_previous: bool,
    pub can_next: bool,
}

impl NativeFaceScene {
    /// Reading pages requires local input, not authority to invoke Body actions.
    /// Drafts/notices keep their existing footer; Patchbay owns separate geometry.
    pub fn page_controls(&self) -> Result<Option<NativePageControls>, FaceSceneError> {
        if !self.local_input_available
            || self.pages() < 2
            || self.showing_diagram
            || self.has_pending_input()
            || self.local_notice.is_some()
        {
            return Ok(None);
        }
        let chrome = chrome::Chrome::prepare_selected(
            self.screen,
            self.text_scale_percent,
            self.typography,
        )?;
        let gap = SPACE_SM;
        let width = chrome
            .footer
            .width
            .checked_sub(gap)
            .ok_or(FaceSceneError::InvalidExtent)?
            / 2;
        let previous = LayoutRect {
            x: chrome.footer.x,
            y: (chrome.bottom + 4) as i16,
            width,
            height: self
                .screen
                .height
                .checked_sub(chrome.bottom + 8)
                .ok_or(FaceSceneError::InvalidExtent)?,
        };
        let next = LayoutRect {
            x: previous.x + (width + gap) as i16,
            ..previous
        };
        Ok(Some(NativePageControls {
            previous,
            next,
            can_previous: self.page > 0,
            can_next: self.page + 1 < self.pages(),
        }))
    }

    pub(super) fn pointer_page(&mut self, x: i16, y: i16) -> Result<Option<bool>, FaceSceneError> {
        let Some(controls) = self.page_controls()? else {
            return Ok(None);
        };
        let contains = |b: LayoutRect| {
            i32::from(x) >= i32::from(b.x)
                && i32::from(y) >= i32::from(b.y)
                && i32::from(x) < i32::from(b.x) + i32::from(b.width)
                && i32::from(y) < i32::from(b.y) + i32::from(b.height)
        };
        let direction = if contains(controls.previous) {
            Some((false, controls.can_previous))
        } else if contains(controls.next) {
            Some((true, controls.can_next))
        } else {
            None
        };
        let Some((forward, available)) = direction else {
            return Ok(None);
        };
        if available {
            self.turn_page(forward);
        }
        Ok(Some(available))
    }

    pub(super) fn append_page_controls(
        &self,
        scene: &mut GraphicsScene,
        controls: NativePageControls,
        compact: bool,
    ) -> Result<(), FaceSceneError> {
        for (bounds, enabled, label) in [
            (
                controls.previous,
                controls.can_previous,
                if compact { "Back" } else { "Previous page" },
            ),
            (
                controls.next,
                controls.can_next,
                if compact { "Next" } else { "Next page" },
            ),
        ] {
            let paint = if enabled {
                GraphicsPaintRole::Accent
            } else {
                GraphicsPaintRole::Muted
            };
            let text = LayoutRect {
                x: bounds.x + 8,
                y: bounds.y + 4,
                width: bounds
                    .width
                    .checked_sub(16)
                    .ok_or(FaceSceneError::InvalidExtent)?,
                height: bounds
                    .height
                    .checked_sub(8)
                    .ok_or(FaceSceneError::InvalidExtent)?,
            };
            let height = TextLayout::new_selected(
                label,
                self.typography.style(GraphicsTextRole::Label.into()),
                text.width,
                self.text_scale_percent,
            )
            .map_err(|_| FaceSceneError::InputBound)?
            .finish_height();
            if height > u32::from(text.height) {
                return Err(FaceSceneError::InputBound);
            }
            push(
                scene,
                GraphicsCommand::rect(bounds, self.screen, paint, GraphicsShapeStyle::Stroke),
            )?;
            push(
                scene,
                GraphicsCommand::text(text, self.screen, paint, label)
                    .and_then(|command| command.with_text_role(GraphicsTextRole::Label)),
            )?;
        }
        Ok(())
    }
}
