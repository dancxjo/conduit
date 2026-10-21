//! Bounded native geometry for the checked three-place Commons graph.
use super::*;
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};

impl Commons {
    pub(in crate::front_door) fn heading(&self) -> Result<String, Error> {
        let source = self.source.borrow();
        let name = ["hub", "workshop", "creche"]
            .iter()
            .find_map(|slot| {
                (place_text(&source.places, slot, "identity").ok()? == self.selected)
                    .then(|| place_text(&source.places, slot, "name").ok())
                    .flatten()
            })
            .ok_or(Error::Face)?;
        Ok(format!("Commons / {name}"))
    }

    pub(in crate::front_door) fn scene(
        &self,
        display: &impl crate::display::PixelTarget,
        journey: &JourneyProjection,
        refusal: Option<&str>,
    ) -> Result<GraphicsScene, Error> {
        let format = display.format().validate().map_err(Error::Display)?;
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width: u16::try_from(format.width).map_err(|_| Error::Scene)?,
            height: u16::try_from(format.height).map_err(|_| Error::Scene)?,
        };
        if screen.width < 480 || screen.height < 480 {
            return self.linear_scene(screen, journey, refusal);
        }
        let mut scene = GraphicsScene::empty();
        rect(
            &mut scene,
            screen,
            screen,
            GraphicsPaintRole::Background,
            GraphicsShapeStyle::Fill,
        )?;
        label(
            &mut scene,
            screen,
            24,
            22,
            &self.heading()?,
            GraphicsTextRole::Title,
            GraphicsPaintRole::Accent,
        )?;
        label(
            &mut scene,
            screen,
            24,
            72,
            journey
                .friendly_name
                .as_deref()
                .unwrap_or("Welcome to Commons"),
            GraphicsTextRole::Body,
            GraphicsPaintRole::Foreground,
        )?;
        label(
            &mut scene,
            screen,
            24,
            110,
            refusal.unwrap_or_else(|| lifecycle_summary(journey)),
            GraphicsTextRole::Status,
            if refusal.is_some() || journey.status == JourneyStatus::InputUnavailable {
                GraphicsPaintRole::Status
            } else {
                GraphicsPaintRole::Muted
            },
        )?;
        let source = self.source.borrow();
        let hub = place_text(&source.places, "hub", "identity")?;
        if self.selected == hub {
            let width = (screen.width - 80) / 3;
            let y = screen.height / 3;
            let height = 120;
            // Each connector depicts an actual authored Hub reachability edge.
            for index in [0, 1] {
                rect(
                    &mut scene,
                    screen,
                    LayoutRect {
                        x: (24 + width + index * (width + 16)) as i16,
                        y: (y + height / 2) as i16,
                        width: 16,
                        height: 2,
                    },
                    GraphicsPaintRole::Muted,
                    GraphicsShapeStyle::Fill,
                )?;
            }
            for (index, slot) in ["workshop", "hub", "creche"].iter().enumerate() {
                let bounds = LayoutRect {
                    x: (24 + index as u16 * (width + 16)) as i16,
                    y: y as i16,
                    width,
                    height,
                };
                let highlighted = index == if self.focus == 0 { 0 } else { 2 };
                let paint = if highlighted {
                    GraphicsPaintRole::Accent
                } else {
                    GraphicsPaintRole::Foreground
                };
                rect(
                    &mut scene,
                    screen,
                    bounds,
                    paint,
                    GraphicsShapeStyle::Stroke,
                )?;
                label(
                    &mut scene,
                    screen,
                    bounds.x + 12,
                    bounds.y + 20,
                    place_text(&source.places, slot, "name")?,
                    GraphicsTextRole::Heading,
                    paint,
                )?;
                if *slot == "hub" {
                    label(
                        &mut scene,
                        screen,
                        bounds.x + 12,
                        bounds.y + 64,
                        "You are here",
                        GraphicsTextRole::Body,
                        GraphicsPaintRole::Muted,
                    )?;
                } else if highlighted {
                    label(
                        &mut scene,
                        screen,
                        bounds.x + 12,
                        bounds.y + 64,
                        "Enter to visit",
                        GraphicsTextRole::Body,
                        GraphicsPaintRole::Accent,
                    )?;
                }
            }
            label(
                &mut scene,
                screen,
                24,
                (y + height + 40) as i16,
                if journey.body_id.is_some() {
                    "Workshop holds your resident Plots."
                } else {
                    "Visit Crèche to birth your Body."
                },
                GraphicsTextRole::Body,
                GraphicsPaintRole::Foreground,
            )?;
            label(
                &mut scene,
                screen,
                24,
                (screen.height - 56) as i16,
                "Arrows choose a place · Enter visit",
                GraphicsTextRole::Body,
                GraphicsPaintRole::Muted,
            )?;
        } else {
            label(
                &mut scene,
                screen,
                24,
                160,
                "This Body is already born.",
                GraphicsTextRole::Heading,
                GraphicsPaintRole::Foreground,
            )?;
            label(
                &mut scene,
                screen,
                24,
                216,
                "Your resident Plots and their state remain in the same Body.",
                GraphicsTextRole::Body,
                GraphicsPaintRole::Foreground,
            )?;
            label(
                &mut scene,
                screen,
                24,
                (screen.height - 56) as i16,
                "Escape returns to Hub",
                GraphicsTextRole::Body,
                GraphicsPaintRole::Muted,
            )?;
        }
        Ok(scene)
    }
    fn linear_scene(
        &self,
        screen: LayoutRect,
        journey: &JourneyProjection,
        refusal: Option<&str>,
    ) -> Result<GraphicsScene, Error> {
        if screen.width < 280 || screen.height < 280 {
            return Err(Error::Scene);
        }
        let mut scene = GraphicsScene::empty();
        rect(
            &mut scene,
            screen,
            screen,
            GraphicsPaintRole::Background,
            GraphicsShapeStyle::Fill,
        )?;
        label(
            &mut scene,
            screen,
            16,
            12,
            &self.heading()?,
            GraphicsTextRole::Heading,
            GraphicsPaintRole::Accent,
        )?;
        let source = self.source.borrow();
        let in_hub = self.selected == place_text(&source.places, "hub", "identity")?;
        if in_hub {
            for (index, slot) in ["workshop", "creche"].iter().enumerate() {
                label(
                    &mut scene,
                    screen,
                    16,
                    64 + index as i16 * 44,
                    &format!(
                        "{} {}",
                        if self.focus == index { ">" } else { " " },
                        place_text(&source.places, slot, "name")?
                    ),
                    GraphicsTextRole::Body,
                    if self.focus == index {
                        GraphicsPaintRole::Accent
                    } else {
                        GraphicsPaintRole::Foreground
                    },
                )?;
            }
        } else {
            label(
                &mut scene,
                screen,
                16,
                64,
                "This Body is already born.",
                GraphicsTextRole::Body,
                GraphicsPaintRole::Foreground,
            )?;
        }
        label(
            &mut scene,
            screen,
            16,
            160,
            refusal.unwrap_or_else(|| lifecycle_summary(journey)),
            GraphicsTextRole::Status,
            GraphicsPaintRole::Status,
        )?;
        label(
            &mut scene,
            screen,
            16,
            (screen.height - 60) as i16,
            if in_hub {
                "Arrows choose · Enter visit · F2 details"
            } else {
                "Escape returns to Hub · F2 details"
            },
            GraphicsTextRole::Body,
            GraphicsPaintRole::Muted,
        )?;
        Ok(scene)
    }
}

fn rect(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    bounds: LayoutRect,
    paint: GraphicsPaintRole,
    style: GraphicsShapeStyle,
) -> Result<(), Error> {
    scene
        .push(GraphicsCommand::rect(bounds, screen, paint, style).map_err(|_| Error::Scene)?)
        .map_err(|_| Error::Scene)
}
fn label(
    scene: &mut GraphicsScene,
    screen: LayoutRect,
    x: i16,
    y: i16,
    text: &str,
    role: GraphicsTextRole,
    paint: GraphicsPaintRole,
) -> Result<(), Error> {
    scene
        .push(
            GraphicsCommand::text(
                LayoutRect {
                    x,
                    y,
                    width: screen.width.saturating_sub(x as u16 + 20),
                    height: 36,
                },
                screen,
                paint,
                text,
            )
            .and_then(|command| command.with_text_role(role))
            .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}
