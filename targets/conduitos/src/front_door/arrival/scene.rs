//! Native layout of the shared Crèche state; geometry belongs to this renderer.
use super::{Arrival, Error};
use crate::display::PixelTarget;
use alloc::format;
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

impl Arrival {
    pub(in crate::front_door) fn scene(
        &self,
        display: &impl PixelTarget,
    ) -> Result<GraphicsScene, Error> {
        let format = display.format().validate().map_err(Error::Display)?;
        let width = u16::try_from(format.width).map_err(|_| Error::Scene)?;
        let height = u16::try_from(format.height).map_err(|_| Error::Scene)?;
        if width < 480 || height < 480 {
            return Err(Error::Scene);
        }
        let screen = LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let card_width = width.saturating_sub(64).min(640);
        let x = i16::try_from((width - card_width) / 2).map_err(|_| Error::Scene)?;
        let y = i16::try_from((height - 448) / 2).map_err(|_| Error::Scene)?;
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
        let mut line = |row: i16, value: &str, focused: bool| {
            let bounds = LayoutRect {
                x,
                y: y + row,
                width: card_width,
                height: 32,
            };
            scene
                .push(
                    GraphicsCommand::text(
                        bounds,
                        screen,
                        if focused {
                            GraphicsPaintRole::Accent
                        } else {
                            GraphicsPaintRole::Foreground
                        },
                        value,
                    )
                    .map_err(|_| Error::Scene)?,
                )
                .map_err(|_| Error::Scene)
        };
        line(0, "CONDUIT / CRÈCHE", false)?;
        line(38, "A Body of your own", false)?;
        line(64, "Give it a name. Choose what it wakes with.", false)?;
        line(108, "BODY NAME", self.focus == 0)?;
        line(
            132,
            &format!(
                "{} {}",
                if self.focus == 0 { ">" } else { " " },
                self.draft.friendly_name()
            ),
            self.focus == 0,
        )?;
        let tradition = self
            .draft
            .naming_systems()
            .find(|(id, _)| *id == self.draft.requested_system())
            .map_or("Surprise me", |(_, label)| label);
        line(
            176,
            &format!(
                "{} Naming tradition: {tradition}",
                if self.focus == 1 { ">" } else { " " }
            ),
            self.focus == 1,
        )?;
        line(204, "  Suggest another name  [F2]", self.focus == 2)?;
        line(248, "FORMS TO INCLUDE", false)?;
        for (index, choice) in self.draft.choices().iter().enumerate() {
            line(
                276 + index as i16 * 24,
                &format!(
                    "{} [{}] {}{}",
                    if self.focus == index + 3 { ">" } else { " " },
                    if choice.selected { "x" } else { " " },
                    choice.title,
                    if choice.refusal.is_some() {
                        " / unavailable"
                    } else {
                        ""
                    }
                ),
                self.focus == index + 3,
            )?;
        }
        line(
            328,
            "  Birth Body and wake Forms  [Enter / F3]",
            self.focus == self.draft.choices().len() + 3,
        )?;
        if let Some(refusal) = &self.refusal {
            line(368, refusal, true)?;
        }
        line(416, "Tab moves  ·  Arrows choose  ·  F9 visits Tour", false)?;
        Ok(scene)
    }
}
