//! Preparation-only viewport shared by generic, configured and mechanical flow.
//! Chrome is measured before rows; no virtual screen or post-layout translation.
use super::*;

pub(in crate::native_face_scene) struct Viewport {
    pub screen: LayoutRect,
    pub scale: u16,
    pub typography: Typography,
    pub chrome: chrome::Chrome,
}
impl Viewport {
    pub fn legacy(
        screen: LayoutRect,
        scale: u16,
        typography: Typography,
    ) -> Result<Self, FaceSceneError> {
        Ok(Self {
            screen,
            scale,
            typography,
            chrome: chrome::Chrome::prepare_selected(screen, scale, typography)?,
        })
    }
    pub fn for_face(
        screen: LayoutRect,
        scale: u16,
        typography: Typography,
        spacing: RowSpacing,
        face: &Face,
        interaction_admitted: bool,
        documents: [&[document::Item]; 2],
    ) -> Result<Self, FaceSceneError> {
        let spacing = spacing.validate()?;
        let mut minimum = 1;
        // Each document is bounded before its scan. Reserve at least one
        // complete line of every actual row kind, including its icons/insets.
        // Oversized keep groups retain their ordinary declared fallback.
        for items in documents {
            field_clusters::validate(items)?;
            for item in items.iter().filter(|item| !item.text.is_empty()) {
                let line = TextLayout::new_selected(
                    "",
                    typography.style(item.role.into()),
                    screen.width,
                    scale,
                )
                .map_err(|_| FaceSceneError::InvalidExtent)?
                .finish_height();
                minimum = minimum.max(spacing.box_height(
                    item,
                    u16::try_from(line).map_err(|_| FaceSceneError::InvalidExtent)?,
                    scale,
                )?);
            }
        }
        if interaction_admitted
            && let Some(editor) =
                text_editor::minimum_extent(face, screen.width, scale, typography)?
        {
            minimum = minimum.max(editor);
        }
        let mut viewport = Self::legacy(screen, scale, typography)?;
        viewport.chrome.reserve_body_name(face, scale, minimum)?;
        Ok(viewport)
    }
}
