//! A single bounded picker over the current Body Play; no second launcher.
use super::*;
use crate::front_door::lifecycle_summary;
use alloc::format;
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};
impl FrontDoor {
    pub(in crate::front_door) fn home_scene(
        &self,
        display: &impl crate::display::PixelTarget,
    ) -> Result<GraphicsScene, Error> {
        let home = self.home.as_ref().ok_or(Error::Scene)?;
        let journey = self.journey.as_ref().ok_or(Error::Scene)?;
        let format = display.format().validate().map_err(Error::Display)?;
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: u16::try_from(format.width).map_err(|_| Error::Scene)?,
            height: u16::try_from(format.height).map_err(|_| Error::Scene)?,
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
                .map_err(|_| Error::Scene)?,
            )
            .map_err(|_| Error::Scene)?;
        draw_text(
            &mut scene,
            screen,
            24,
            24,
            &home.commons.heading()?,
            GraphicsTextRole::Title,
            GraphicsPaintRole::Accent,
        )?;
        draw_text(
            &mut scene,
            screen,
            24,
            70,
            journey.friendly_name.as_deref().unwrap_or("Welcome"),
            GraphicsTextRole::Body,
            GraphicsPaintRole::Foreground,
        )?;
        let plots = self.home_active_plots();
        if journey.body_id.is_none() {
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
                let focused = index == home.model.selected_plot_index();
                draw_text(
                    &mut scene,
                    screen,
                    24,
                    y,
                    &format!(
                        "{} {}{}",
                        if focused { ">" } else { " " },
                        plot.title,
                        if plot.foreground { "  ·  current" } else { "" }
                    ),
                    GraphicsTextRole::Heading,
                    if focused {
                        GraphicsPaintRole::Accent
                    } else {
                        GraphicsPaintRole::Foreground
                    },
                )?;
            }
        }
        if let Some(refusal) = &self.refusal {
            draw_text(
                &mut scene,
                screen,
                24,
                370,
                &format!("{} {}", refusal.heading(), refusal.reason()),
                GraphicsTextRole::Warning,
                GraphicsPaintRole::Status,
            )?;
        }
        let footer = if journey.body_id.is_none() {
            "Enter · Birth"
        } else if plots.is_empty() {
            lifecycle_summary(journey)
        } else {
            "Up/Down · choose a plot    Enter · open    Esc · return"
        };
        draw_text(
            &mut scene,
            screen,
            24,
            i16::try_from(screen.height.saturating_sub(48)).map_err(|_| Error::Scene)?,
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
) -> Result<(), Error> {
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
                .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}
