//! Content-sized labeled fields, bounded by the native scene and scroll budgets.
use super::TourShellError;
use conduit_presentation::{GraphicsScene, LayoutRect, MAX_GRAPHICS_COMMANDS, Presentation};

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
        let label_height = crate::display::text_height(&subject.label, width)
            .map_err(|_| TourShellError::Scene)?;
        let value_height =
            crate::display::text_height(&item.text, width).map_err(|_| TourShellError::Scene)?;
        let height = label_height
            .checked_add(value_height)
            .ok_or(TourShellError::Scene)?;
        let y = i32::from(next_y) - i32::from(scroll_y);
        next_y = next_y
            .checked_add(height)
            .and_then(|end| end.checked_add(12))
            .filter(|end| *end <= super::scroll::MAX_SCROLL_CONTENT_HEIGHT)
            .ok_or(TourShellError::Scene)?;
        // Validate the payload even when this field is currently off screen.
        let commands = crate::native_components::labeled_field(
            LayoutRect {
                x: 12,
                y: i16::try_from(y).map_err(|_| TourShellError::Scene)?,
                width,
                height,
            },
            viewport,
            &subject.label,
            &item.text,
        )
        .map_err(|_| TourShellError::Scene)?;
        if y + i32::from(height) > 36
            && y < i32::from(bounds.height)
            && let Some(scene) = scene.as_deref_mut()
        {
            if scene.commands().len() > MAX_GRAPHICS_COMMANDS - commands.len() {
                return Err(TourShellError::Scene);
            }
            for command in commands {
                scene.push(command).map_err(|_| TourShellError::Scene)?;
            }
        }
    }
    Ok(next_y)
}
