//! Native layout of the shared Crèche state; geometry belongs to this renderer.
use super::{Arrival, Error};
use crate::display::{PixelTarget, SPACING};
use alloc::format;
use conduit_presentation::{
    ActionAvailability, FieldKind, GraphicsCommand, GraphicsPaintRole, GraphicsScene,
    GraphicsShapeStyle, GraphicsTextRole, LayoutRect, PresentationIconKey,
    PresentationMechanism,
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
        let card_width = width.saturating_sub(SPACING[4] * 2).min(640);
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
        let mut line = |row: i16, value: &str, focused: bool, typography: GraphicsTextRole| {
            let bounds = LayoutRect {
                x,
                y: y + row,
                width: card_width,
                height: 32,
            };
            let paint = match typography {
                GraphicsTextRole::Heading | GraphicsTextRole::Action | GraphicsTextRole::Title => {
                    GraphicsPaintRole::Accent
                }
                GraphicsTextRole::Code | GraphicsTextRole::Status | GraphicsTextRole::Warning => {
                    if typography == GraphicsTextRole::Warning {
                        GraphicsPaintRole::Warning
                    } else {
                        GraphicsPaintRole::Status
                    }
                }
                GraphicsTextRole::Muted => GraphicsPaintRole::Muted,
                _ => GraphicsPaintRole::Foreground,
            };
            let paint = if focused {
                GraphicsPaintRole::Accent
            } else {
                paint
            };
            scene
                .push(
                    GraphicsCommand::text(bounds, screen, paint, value)
                        .and_then(|command| command.with_text_role(typography))
                        .map_err(|_| Error::Scene)?,
                )
                .map_err(|_| Error::Scene)?;
            if focused {
                let frame = LayoutRect {
                    x: bounds.x - 8,
                    y: bounds.y - 2,
                    width: bounds.width + 16,
                    height: 32,
                };
                scene
                    .push(
                        GraphicsCommand::rect(
                            frame,
                            screen,
                            GraphicsPaintRole::Accent,
                            GraphicsShapeStyle::Stroke,
                        )
                        .map_err(|_| Error::Scene)?,
                    )
                    .map_err(|_| Error::Scene)?;
            }
            Ok::<(), Error>(())
        };
        line(0, "Conduit / Crèche", false, GraphicsTextRole::Muted)?;
        let view = self.draft.presentation().map_err(|_| Error::Presentation)?;
        let mut choice_count = 0;
        for node in &view.root.children {
            match (&*node.key, &node.mechanism) {
                ("creche-heading", PresentationMechanism::Heading { text }) => {
                    line(30, text, false, GraphicsTextRole::Title)?
                }
                ("body-name", PresentationMechanism::FormField(field)) => {
                    line(72, &field.help, false, GraphicsTextRole::Body)?;
                    line(108, &field.label, false, GraphicsTextRole::Label)?;
                    line(
                        132,
                        &field.value,
                        self.focus == 0,
                        GraphicsTextRole::Heading,
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
                        &format!("{}: {}", field.label, selected.label),
                        self.focus == 1,
                        GraphicsTextRole::Label,
                    )?;
                }
                ("suggest-name", PresentationMechanism::Action(action)) => {
                    line(
                        208,
                        &format!("{}  ·  F2", action.label),
                        self.focus == 2,
                        GraphicsTextRole::Action,
                    )?;
                }
                ("initial-forms", PresentationMechanism::ChoiceGroup { label, options, .. }) => {
                    line(248, label, false, GraphicsTextRole::Label)?;
                    for (index, choice) in options.iter().enumerate() {
                        line(
                            276 + index as i16 * 24,
                            &format!(
                                "{}  {}{}",
                                if choice.selected { "●" } else { "○" },
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
                            GraphicsTextRole::Body,
                        )?;
                    }
                    choice_count = options.len();
                }
                ("birth-body", PresentationMechanism::Action(action)) => {
                    line(
                        328,
                        &format!("{}  ·  Enter / F3", action.label),
                        self.focus == choice_count + 3,
                        GraphicsTextRole::Action,
                    )?;
                }
                _ => return Err(Error::Scene),
            }
        }
        if let Some(refusal) = &self.refusal {
            line(368, refusal, true, GraphicsTextRole::Warning)?;
            scene
                .push(
                    GraphicsCommand::icon(
                        LayoutRect { x: x - 24, y: y + 368, width: 16, height: 16 },
                        screen,
                        GraphicsPaintRole::Warning,
                        PresentationIconKey::Warning,
                    )
                    .map_err(|_| Error::Scene)?,
                )
                .map_err(|_| Error::Scene)?;
        }
        scene
            .push(
                GraphicsCommand::icon(
                    LayoutRect { x: x - 24, y: y + 328, width: 16, height: 16 },
                    screen,
                    GraphicsPaintRole::Success,
                    PresentationIconKey::Confirm,
                )
                .map_err(|_| Error::Scene)?,
            )
            .map_err(|_| Error::Scene)?;
        line(
            416,
            "Tab moves  ·  Arrows choose  ·  F9 visits Tour",
            false,
            GraphicsTextRole::Muted,
        )?;
        Ok(scene)
    }
}
