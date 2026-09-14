//! Small native rendering vocabulary, with no action or runtime ownership.

#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
use conduit_presentation::{
    GraphicsCommand, GraphicsError, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle,
    LayoutRect, MAX_GRAPHICS_COMMANDS, PresentationIconKey,
};

/// Append one button atomically within the admitted scene budget.
/// The caller owns its semantic action, hit target, and containing clip.
#[cfg(any(test, all(target_arch = "x86_64", feature = "native-compositor")))]
pub(crate) fn action_button(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    clip: LayoutRect,
    label: &str,
    icon: Option<PresentationIconKey>,
) -> Result<(), GraphicsError> {
    let command_count = if icon.is_some() { 3 } else { 2 };
    if scene.commands().len() > MAX_GRAPHICS_COMMANDS - command_count {
        return Err(GraphicsError::TooManyCommands);
    }
    let inset = crate::display::SPACE_SM;
    let icon_extent = crate::display::ICON_SM;
    let text_leading = if icon.is_some() {
        inset + icon_extent + inset
    } else {
        inset
    };
    let border = GraphicsCommand::rect(
        bounds,
        clip,
        GraphicsPaintRole::Accent,
        GraphicsShapeStyle::RoundedStroke,
    )?;
    let text = GraphicsCommand::text(
        LayoutRect {
            x: bounds
                .x
                .checked_add(
                    i16::try_from(text_leading).map_err(|_| GraphicsError::InvalidGeometry)?,
                )
                .ok_or(GraphicsError::InvalidGeometry)?,
            y: bounds
                .y
                .checked_add(
                    i16::try_from(crate::display::SPACE_XS)
                        .map_err(|_| GraphicsError::InvalidGeometry)?,
                )
                .ok_or(GraphicsError::InvalidGeometry)?,
            width: bounds.width.saturating_sub(text_leading + inset).max(1),
            height: bounds
                .height
                .saturating_sub(crate::display::SPACE_SM)
                .max(1),
        },
        clip,
        GraphicsPaintRole::Foreground,
        label,
    )?;
    let icon = icon
        .map(|icon| {
            GraphicsCommand::icon(
                LayoutRect {
                    x: bounds
                        .x
                        .checked_add(
                            i16::try_from(inset).map_err(|_| GraphicsError::InvalidGeometry)?,
                        )
                        .ok_or(GraphicsError::InvalidGeometry)?,
                    y: bounds
                        .y
                        .checked_add(
                            i16::try_from(crate::display::SPACE_XS)
                                .map_err(|_| GraphicsError::InvalidGeometry)?,
                        )
                        .ok_or(GraphicsError::InvalidGeometry)?,
                    width: icon_extent,
                    height: icon_extent,
                },
                clip,
                GraphicsPaintRole::Accent,
                icon,
            )
        })
        .transpose()?;
    // Every command is validated before any is committed. Capacity was checked
    // above, so no push can fail after a partial component.
    scene.push(border)?;
    if let Some(icon) = icon {
        scene.push(icon)?;
    }
    scene.push(text)
}

#[cfg(test)]
fn button(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    clip: LayoutRect,
    label: &str,
) -> Result<(), GraphicsError> {
    action_button(scene, bounds, clip, label, None)
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
        assert_eq!(scene.commands()[0].style, GraphicsShapeStyle::RoundedStroke);
        assert!(
            crate::display::text_height(text.payload(), text.bounds.width).unwrap()
                <= text.bounds.height
        );
    }

    #[test]
    fn icon_button_uses_the_same_rounded_tokenized_component() {
        let mut scene = GraphicsScene::empty();
        action_button(
            &mut scene,
            BOUNDS,
            BOUNDS,
            "Close",
            Some(PresentationIconKey::Close),
        )
        .unwrap();
        assert_eq!(scene.commands().len(), 3);
        assert_eq!(scene.commands()[0].style, GraphicsShapeStyle::RoundedStroke);
        assert_eq!(scene.commands()[1].bounds.width, crate::display::ICON_SM);
        assert_eq!(
            scene.commands()[1].bounds.x,
            crate::display::SPACE_SM as i16
        );
        assert_eq!(scene.commands()[2].payload(), "Close");
        assert_eq!(
            scene.commands()[2].bounds.x,
            (crate::display::SPACE_SM * 2 + crate::display::ICON_SM) as i16
        );
    }
}
