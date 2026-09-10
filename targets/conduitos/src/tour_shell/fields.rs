//! Content-sized labeled fields, bounded by the native scene and scroll budgets.
use super::TourShellError;
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, LayoutRect, Presentation,
};

pub(super) fn project(
    bounds: LayoutRect,
    presentation: &Presentation,
    scroll_y: u16,
    mut scene: Option<&mut GraphicsScene>,
) -> Result<u16, TourShellError> {
    let width = bounds
        .width
        .checked_sub(24)
        .filter(|width| *width >= 16)
        .ok_or(TourShellError::Scene)?;
    let viewport = LayoutRect {
        x: 0,
        y: 36,
        width: bounds.width,
        height: bounds.height.checked_sub(36).ok_or(TourShellError::Scene)?,
    };
    let mut next_y = 44_u16;
    for item in &presentation.text {
        let subject = presentation
            .subjects
            .iter()
            .find(|subject| subject.identity == item.subject)
            .ok_or(TourShellError::Identity)?;
        let text = alloc::format!("{}\n{}", subject.label, item.text);
        #[cfg(not(feature = "native-compositor"))]
        let height =
            crate::display::text_height(&text, width).map_err(|_| TourShellError::Scene)?;
        #[cfg(feature = "native-compositor")]
        let height = u16::try_from(
            crate::display::typography::TextLayout::new(
                &text,
                crate::display::typography::TextRole::Body,
                width,
            )
            .map_err(|_| TourShellError::Scene)?
            .finish_height(),
        )
        .map_err(|_| TourShellError::Scene)?;
        let y = i32::from(next_y) - i32::from(scroll_y);
        next_y = next_y
            .checked_add(height)
            .and_then(|end| end.checked_add(12))
            .filter(|end| *end <= super::scroll::MAX_SCROLL_CONTENT_HEIGHT)
            .ok_or(TourShellError::Scene)?;
        // Validate the payload even when this field is currently off screen.
        let command = GraphicsCommand::text(
            LayoutRect {
                x: 12,
                y: i16::try_from(y).map_err(|_| TourShellError::Scene)?,
                width,
                height,
            },
            viewport,
            GraphicsPaintRole::Foreground,
            &text,
        )
        .map_err(|_| TourShellError::Scene)?;
        if y + i32::from(height) > 36
            && y < i32::from(bounds.height)
            && let Some(scene) = scene.as_deref_mut()
        {
            scene.push(command).map_err(|_| TourShellError::Scene)?;
        }
    }
    Ok(next_y)
}
