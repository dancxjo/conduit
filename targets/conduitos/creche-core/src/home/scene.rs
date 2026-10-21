//! A single bounded picker over the current Body Play; no second launcher.
use super::{Controller, presentation::Request};
use crate::wire::Refusal;
use alloc::format;
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};
impl Controller {
    pub(crate) fn scene(&self, input: &Request) -> Result<GraphicsScene, Refusal> {
        let extent = input.extent.as_ref().ok_or(Refusal::Bounds)?;
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: extent.width,
            height: extent.height,
        };
        let mut scene = GraphicsScene::empty();
        scene
            .push(
                GraphicsCommand::rect(
                    screen,
                    screen,
                    GraphicsPaintRole::Background,
                    GraphicsShapeStyle::Fill,
                )
                .map_err(|_| Refusal::Projection)?,
            )
            .map_err(|_| Refusal::Projection)?;
        draw_text(
            &mut scene,
            screen,
            24,
            24,
            &input.heading,
            GraphicsTextRole::Title,
            GraphicsPaintRole::Accent,
        )?;
        draw_text(
            &mut scene,
            screen,
            24,
            70,
            input.friendly_name.as_deref().unwrap_or("Welcome"),
            GraphicsTextRole::Body,
            GraphicsPaintRole::Foreground,
        )?;
        let plots = &input.plots;
        if input.birth_available {
            draw_text(
                &mut scene,
                screen,
                24,
                122,
                "Birth a Body to begin.",
                GraphicsTextRole::Heading,
                GraphicsPaintRole::Foreground,
            )?;
        } else if plots.is_empty() {
            draw_text(
                &mut scene,
                screen,
                24,
                122,
                "No active plots.",
                GraphicsTextRole::Heading,
                GraphicsPaintRole::Foreground,
            )?;
        } else {
            for (index, plot) in plots.iter().enumerate() {
                let y = 122 + index as i16 * 56;
                let focused = index == self.model.selected_plot_index();
                draw_text(
                    &mut scene,
                    screen,
                    24,
                    y,
                    &format!("{} {}{}", if focused { ">" } else { " " }, plot.title, ""),
                    GraphicsTextRole::Heading,
                    if focused {
                        GraphicsPaintRole::Accent
                    } else {
                        GraphicsPaintRole::Foreground
                    },
                )?;
            }
        }
        if let Some(refusal) = &input.root_refusal {
            draw_text(
                &mut scene,
                screen,
                24,
                370,
                refusal,
                GraphicsTextRole::Warning,
                GraphicsPaintRole::Status,
            )?;
        }
        let footer = if input.birth_available {
            "Enter · Birth"
        } else if plots.is_empty() {
            &input.idle_status
        } else {
            "Up/Down · choose a plot    Enter · open    Esc · return"
        };
        draw_text(
            &mut scene,
            screen,
            24,
            i16::try_from(screen.height.saturating_sub(48)).map_err(|_| Refusal::Projection)?,
            footer,
            GraphicsTextRole::Body,
            GraphicsPaintRole::Muted,
        )?;
        Ok(scene)
    }
}
fn draw_text(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    x: i16,
    y: i16,
    value: &str,
    role: GraphicsTextRole,
    paint: GraphicsPaintRole,
) -> Result<(), Refusal> {
    let bounds = LayoutRect {
        x,
        y,
        width: screen.width.saturating_sub(x as u16 + 24),
        height: 44,
    };
    scene
        .push(
            GraphicsCommand::text(bounds, screen, paint, value)
                .and_then(|command| command.with_text_role(role))
                .map_err(|_| Refusal::Projection)?,
        )
        .map_err(|_| Refusal::Projection)
}
