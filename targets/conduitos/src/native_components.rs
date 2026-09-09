//! Small native rendering vocabulary, with no action or runtime ownership.

use conduit_presentation::{
    GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle,
    LayoutRect, MAX_GRAPHICS_COMMANDS,
};

/// Resolve a content-sized field into two bounded commands. Labels and values
/// keep separate paint roles; the caller retains scrolling and semantic truth.
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn labeled_field(
    bounds: LayoutRect,
    clip: LayoutRect,
    label: &str,
    value: &str,
) -> Result<[GraphicsCommand; 2], GraphicsError> {
    let label_height = crate::display::text_height(label, bounds.width)
        .map_err(|_| GraphicsError::InvalidGeometry)?;
    let value_height = bounds
        .height
        .checked_sub(label_height)
        .filter(|height| *height > 0)
        .ok_or(GraphicsError::InvalidGeometry)?;
    Ok([
        GraphicsCommand::text(
            LayoutRect {
                height: label_height,
                ..bounds
            },
            clip,
            GraphicsPaintRole::Accent,
            label,
        )?,
        GraphicsCommand::text(
            LayoutRect {
                y: bounds
                    .y
                    .checked_add(
                        i16::try_from(label_height).map_err(|_| GraphicsError::InvalidGeometry)?,
                    )
                    .ok_or(GraphicsError::InvalidGeometry)?,
                height: value_height,
                ..bounds
            },
            clip,
            GraphicsPaintRole::Foreground,
            value,
        )?,
    ])
}

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
    fn field_keeps_label_and_exact_value_separate_under_one_clip() {
        let bounds = LayoutRect {
            width: 80,
            height: 64,
            ..BOUNDS
        };
        let commands = labeled_field(bounds, bounds, "Current state", "\"hello\"").unwrap();
        assert_eq!(commands[0].payload(), "Current state");
        assert_eq!(commands[0].paint, GraphicsPaintRole::Accent);
        assert_eq!(commands[0].bounds.height, 32);
        assert_eq!(commands[1].payload(), "\"hello\"");
        assert_eq!(commands[1].paint, GraphicsPaintRole::Foreground);
        assert_eq!(commands[1].bounds.y, 32);
        assert!(commands.iter().all(|command| command.clip == bounds));
    }

    #[test]
    fn field_refuses_insufficient_height_and_coordinate_overflow() {
        assert!(
            labeled_field(
                LayoutRect {
                    height: 16,
                    ..BOUNDS
                },
                BOUNDS,
                "Kind",
                "text/upper"
            )
            .is_err()
        );
        assert!(
            labeled_field(
                LayoutRect {
                    y: i16::MAX,
                    height: 32,
                    ..BOUNDS
                },
                BOUNDS,
                "Kind",
                "text/upper"
            )
            .is_err()
        );
    }

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
