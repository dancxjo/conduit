//! Native layout of the shared Crèche state; geometry belongs to this renderer.
use super::{Arrival, Error};
use crate::display::PixelTarget;
use alloc::{format, string::String};
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
        let controls = self.controls();
        let focused = controls.get(self.focus).map(String::as_str);
        for node in &view.root.children {
            match (&*node.key, &node.mechanism) {
                ("creche-heading", PresentationMechanism::Heading { text }) => {
                    line(28, text, false)?
                }
                ("body-name", PresentationMechanism::FormField(field)) => {
                    line(52, &field.help, false)?;
                    line(92, &field.label, self.focus == 0)?;
                    line(
                        116,
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
                        156,
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
                    line(184, &format!("  {}  [F2]", action.label), self.focus == 2)?;
                }
                ("initial-forms", PresentationMechanism::ChoiceGroup { label, options, .. }) => {
                    line(260, label, false)?;
                    for (index, choice) in options.iter().enumerate() {
                        line(
                            288 + index as i16 * 24,
                            &format!(
                                "{} [{}] {}{}",
                                if focused == Some(choice.change_action.identity.as_str()) {
                                    ">"
                                } else {
                                    " "
                                },
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
                            focused == Some(choice.change_action.identity.as_str()),
                        )?;
                    }
                }
                ("birth-body", PresentationMechanism::Action(action)) => {
                    line(
                        372,
                        &format!("  {}  [Enter / F3]", action.label),
                        focused == Some(action.identity.as_str()),
                    )?;
                }
                ("form-search", PresentationMechanism::FormField(field)) => {
                    line(
                        224,
                        &format!(
                            "{} {}: {}",
                            if focused == Some(field.input_action.identity.as_str()) {
                                ">"
                            } else {
                                " "
                            },
                            field.label,
                            field.value
                        ),
                        focused == Some(field.input_action.identity.as_str()),
                    )?;
                }
                ("selected-forms", PresentationMechanism::Status { title, .. }) => {
                    line(340, title, false)?
                }
                ("initial-forms", PresentationMechanism::Status { title, .. }) => {
                    line(288, title, false)?
                }
                _ => return Err(Error::Scene),
            }
        }
        if let Some(refusal) = &self.refusal {
            line(392, refusal, true)?;
        }
        line(416, "Tab moves  ·  Arrows choose  ·  F9 visits Tour", false)?;
        Ok(scene)
    }
}
