//! Small native rendering vocabulary, with no action or runtime ownership.

use conduit_presentation::{
    GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle,
    LayoutRect, MAX_GRAPHICS_COMMANDS,
};

/// Append one button atomically within the admitted scene budget.
/// The caller owns its semantic action, hit target, and containing clip.
pub(crate) fn button(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    clip: LayoutRect,
    label: &str,
) -> Result<(), GraphicsError> {
    if scene.commands().len() > MAX_GRAPHICS_COMMANDS - 2 {
        return Err(GraphicsError::TooManyCommands);
    }
    let border = GraphicsCommand::rect(
        bounds,
        clip,
        GraphicsPaintRole::Accent,
        GraphicsShapeStyle::Stroke,
    )?;
    let text = GraphicsCommand::text(
        LayoutRect {
            x: bounds
                .x
                .checked_add(8)
                .ok_or(GraphicsError::InvalidGeometry)?,
            y: bounds
                .y
                .checked_add(4)
                .ok_or(GraphicsError::InvalidGeometry)?,
            width: bounds.width.saturating_sub(16).max(1),
            height: bounds.height.saturating_sub(8).max(1),
        },
        clip,
        GraphicsPaintRole::Foreground,
        label,
    )?;
    // Both commands are validated before either is committed. Capacity was
    // checked above, so neither push can fail after a partial component.
    scene.push(border)?;
    scene.push(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: LayoutRect = LayoutRect {
        x: 0,
        y: 0,
        width: 112,
        height: 28,
    };

    #[test]
    fn button_admission_is_atomic_for_capacity_and_invalid_text() {
        let mut scene = GraphicsScene::empty();
        let fill = GraphicsCommand::rect(
            BOUNDS,
            BOUNDS,
            GraphicsPaintRole::Background,
            GraphicsShapeStyle::Fill,
        )
        .unwrap();
        for _ in 0..MAX_GRAPHICS_COMMANDS - 1 {
            scene.push(fill).unwrap();
        }
        let before = scene;
        assert_eq!(
            button(&mut scene, BOUNDS, BOUNDS, "Gears"),
            Err(GraphicsError::TooManyCommands)
        );
        assert_eq!(scene, before);
        let mut scene = GraphicsScene::empty();
        let before = scene;
        assert!(button(&mut scene, BOUNDS, BOUNDS, "").is_err());
        assert_eq!(scene, before);
    }

    #[test]
    fn button_reserves_a_complete_font_line_in_exactly_two_commands() {
        let mut scene = GraphicsScene::empty();
        button(&mut scene, BOUNDS, BOUNDS, "Gears").unwrap();
        assert_eq!(scene.commands().len(), 2);
        let text = &scene.commands()[1];
        assert!(
            crate::display::text_height(text.payload(), text.bounds.width).unwrap()
                <= text.bounds.height
        );
    }
}
