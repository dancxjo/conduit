//! Native layout of the shared Crèche state; geometry belongs to this renderer.
use super::{Arrival, Error};
use crate::display::PixelTarget;
use alloc::format;
use conduit_presentation::{
    ActionAvailability, FieldKind, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, LayoutRect, PresentationMechanism,
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
        let view = self.draft.presentation().map_err(|_| Error::Presentation)?;
        let mut choice_count = 0;
        for node in &view.root.children {
            match (&*node.key, &node.mechanism) {
                ("creche-heading", PresentationMechanism::Heading { text }) => {
                    line(38, text, false)?
                }
                ("body-name", PresentationMechanism::FormField(field)) => {
                    line(64, &field.help, false)?;
                    line(108, &field.label, self.focus == 0)?;
                    line(
                        132,
                        &format!(
                            "{} {}",
                            if self.focus == 0 { ">" } else { " " },
                            field.value
                        ),
                        self.focus == 0,
                    )?;
                }
                ("name-system", PresentationMechanism::FormField(field)) => {
                    let FieldKind::NamedSelect { options } = &field.kind else {
                        return Err(Error::Scene);
                    };
                    let selected = options
                        .iter()
                        .find(|option| option.identity == field.value)
                        .ok_or(Error::Scene)?;
                    line(
                        176,
                        &format!(
                            "{} {}: {}",
                            if self.focus == 1 { ">" } else { " " },
                            field.label,
                            selected.label
                        ),
                        self.focus == 1,
                    )?;
                }
                ("suggest-name", PresentationMechanism::Action(action)) => {
                    line(204, &format!("  {}  [F2]", action.label), self.focus == 2)?;
                }
                ("initial-forms", PresentationMechanism::ChoiceGroup { label, options, .. }) => {
                    line(248, label, false)?;
                    for (index, choice) in options.iter().enumerate() {
                        line(
                            276 + index as i16 * 24,
                            &format!(
                                "{} [{}] {}{}",
                                if self.focus == index + 3 { ">" } else { " " },
                                if choice.selected { "x" } else { " " },
                                choice.label,
                                if matches!(
                                    choice.change_action.availability,
                                    ActionAvailability::Available
                                ) {
                                    ""
                                } else {
                                    " / unavailable"
                                }
                            ),
                            self.focus == index + 3,
                        )?;
                    }
                    choice_count = options.len();
                }
                ("birth-body", PresentationMechanism::Action(action)) => {
                    line(
                        328,
                        &format!("  {}  [Enter / F3]", action.label),
                        self.focus == choice_count + 3,
                    )?;
                }
                _ => return Err(Error::Scene),
            }
        }
        if let Some(refusal) = &self.refusal {
            line(368, refusal, true)?;
        }
        line(416, "Tab moves  ·  Arrows choose  ·  F9 visits Tour", false)?;
        Ok(scene)
    }
}
