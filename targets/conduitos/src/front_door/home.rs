//! Finite ConduitOS Home and command-bar presentation state.

use alloc::format;
pub use conduit_home_model::{HomeAction as HomeInput, HomeView};
use conduit_home_model::{HomeEvent, HomeModel};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyTransition, KeymapDisposition};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};

use super::{Error, FrontDoor};

pub(super) struct Home {
    model: HomeModel,
    keymap: ConduitIntlKeymap,
}

impl Home {
    fn new() -> Self {
        Self {
            model: HomeModel::new(),
            keymap: ConduitIntlKeymap::new(),
        }
    }

    fn accept(&mut self, event: KeyEvent) -> HomeInput {
        if event.transition() != KeyTransition::Pressed {
            self.keymap.apply(event);
            return HomeInput::Unchanged;
        }
        let forms = installed_form_titles();
        let abstract_event = match event.usage() {
            79 | 81 | 43 => HomeEvent::Next,
            80 | 82 => HomeEvent::Previous,
            40 => HomeEvent::Activate,
            41 => HomeEvent::Escape,
            42 => HomeEvent::Backspace,
            _ => match self.keymap.apply(event) {
                KeymapDisposition::Text(fragment) => {
                    return core::str::from_utf8(fragment.as_bytes())
                        .map_or(HomeInput::Unchanged, |text| {
                            self.model.accept(HomeEvent::Text(text), &forms)
                        });
                }
                KeymapDisposition::Refused(_) => HomeEvent::CharacterUnavailable,
                _ => return HomeInput::Unchanged,
            },
        };
        self.model.accept(abstract_event, &forms)
    }
}

fn installed_form_titles() -> [&'static str; crate::native_workset::NATIVE_FORM_CAPACITY] {
    crate::native_workset::inventory().map(crate::native_workset::NativeForm::title)
}

impl FrontDoor {
    pub(super) fn home_scene(
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
            34,
            28,
            "CONDUIT",
            GraphicsTextRole::Title,
            GraphicsPaintRole::Accent,
        )?;
        draw_text(
            &mut scene,
            screen,
            34,
            60,
            journey.friendly_name.as_deref().unwrap_or("Current Body"),
            GraphicsTextRole::Status,
            GraphicsPaintRole::Muted,
        )?;
        match home.model.view() {
            HomeView::Launcher => {
                for (index, item) in conduit_home_model::HomeDestination::ALL.iter().enumerate() {
                    let column = index % 3;
                    let row = index / 3;
                    let bounds = LayoutRect {
                        x: 120 + column as i16 * 300,
                        y: 130 + row as i16 * 190,
                        width: 220,
                        height: 132,
                    };
                    scene
                        .push(
                            GraphicsCommand::rect(
                                bounds,
                                screen,
                                if index == home.model.selected_index() {
                                    GraphicsPaintRole::Accent
                                } else {
                                    GraphicsPaintRole::Foreground
                                },
                                GraphicsShapeStyle::Stroke,
                            )
                            .map_err(|_| Error::Scene)?,
                        )
                        .map_err(|_| Error::Scene)?;
                    draw_text(
                        &mut scene,
                        screen,
                        bounds.x + 22,
                        bounds.y + 48,
                        item.label(),
                        GraphicsTextRole::Heading,
                        if index == home.model.selected_index() {
                            GraphicsPaintRole::Accent
                        } else {
                            GraphicsPaintRole::Foreground
                        },
                    )?;
                }
                draw_text(
                    &mut scene,
                    screen,
                    34,
                    548,
                    ">  type anywhere for Prompt",
                    GraphicsTextRole::Code,
                    GraphicsPaintRole::Status,
                )?;
            }
            HomeView::Prompt => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    142,
                    "CONDUIT PROMPT",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    214,
                    &format!("conduit> {}_", home.model.command()),
                    GraphicsTextRole::Code,
                    GraphicsPaintRole::Foreground,
                )?;
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    282,
                    home.model.output(),
                    GraphicsTextRole::Body,
                    GraphicsPaintRole::Status,
                )?;
            }
            HomeView::Forms => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    128,
                    "INSTALLED FORMS",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                for (index, form) in crate::native_workset::inventory().iter().enumerate() {
                    draw_text(
                        &mut scene,
                        screen,
                        72,
                        188 + index as i16 * 48,
                        &format!(
                            "{}  {}",
                            if index == home.model.selected_form_index() {
                                ">"
                            } else {
                                " "
                            },
                            form.title()
                        ),
                        GraphicsTextRole::Body,
                        GraphicsPaintRole::Foreground,
                    )?;
                }
            }
            HomeView::Body => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    128,
                    "BODY",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                for (index, line) in [
                    format!(
                        "Name       {}",
                        journey.friendly_name.as_deref().unwrap_or("Current Body")
                    ),
                    format!(
                        "Wake       {}",
                        journey.wake_id.as_ref().map_or("-", |id| id.as_str())
                    ),
                    format!(
                        "Plan       {}",
                        journey.plan_id.as_ref().map_or("idle", |id| id.as_str())
                    ),
                    format!(
                        "Play       {}",
                        journey
                            .active_play_id
                            .as_ref()
                            .map_or("-", |id| id.as_str())
                    ),
                    format!("Lines      {}", usize::from(self.connectivity.is_some())),
                    format!("Host       {}", journey.host_id.as_str()),
                ]
                .iter()
                .enumerate()
                {
                    draw_text(
                        &mut scene,
                        screen,
                        72,
                        188 + index as i16 * 44,
                        line,
                        GraphicsTextRole::Code,
                        GraphicsPaintRole::Foreground,
                    )?;
                }
            }
        }
        let status_y = i16::try_from(screen.height.saturating_sub(52)).map_err(|_| Error::Scene)?;
        scene
            .push(
                GraphicsCommand::rect(
                    LayoutRect {
                        x: 0,
                        y: status_y - 12,
                        width: screen.width,
                        height: 64,
                    },
                    screen,
                    GraphicsPaintRole::Status,
                    GraphicsShapeStyle::Stroke,
                )
                .map_err(|_| Error::Scene)?,
            )
            .map_err(|_| Error::Scene)?;
        draw_text(
            &mut scene,
            screen,
            24,
            status_y,
            &format!(
                "BODY {}  |  WAKE {}  |  PLAN {}  |  PLAY {}  |  LINES {}  |  HOST {}",
                journey.friendly_name.as_deref().unwrap_or("CURRENT"),
                if journey.wake_id.is_some() { "1" } else { "-" },
                if journey.plan_id.is_some() {
                    "current"
                } else {
                    "idle"
                },
                if journey.active_play_id.is_some() {
                    "current"
                } else {
                    "-"
                },
                usize::from(self.connectivity.is_some()),
                journey.host_id.as_str()
            ),
            GraphicsTextRole::Status,
            GraphicsPaintRole::Status,
        )?;
        Ok(scene)
    }

    pub fn open_home(&mut self) -> Result<(), Error> {
        if self
            .journey
            .as_ref()
            .is_none_or(|journey| journey.body_id.is_none())
        {
            return Err(Error::Presentation);
        }
        self.home = Some(Home::new());
        self.advance()
    }

    pub fn home_open(&self) -> bool {
        self.home.is_some()
    }

    pub fn close_home(&mut self) -> Result<(), Error> {
        if self.home.take().is_none() {
            return Err(Error::Presentation);
        }
        self.advance()
    }

    pub fn home_view(&self) -> Option<HomeView> {
        self.home.as_ref().map(|home| home.model.view())
    }

    pub fn home_selection(&self) -> Option<usize> {
        self.home.as_ref().map(|home| home.model.selected_index())
    }

    pub fn home_application_view(
        &self,
    ) -> Result<Option<conduit_presentation::ApplicationView>, Error> {
        self.home
            .as_ref()
            .map(|home| {
                home.model
                    .presentation(
                        u32::try_from(self.revision).map_err(|_| Error::Presentation)?,
                        &installed_form_titles(),
                    )
                    .lower()
                    .map_err(|_| Error::Presentation)
            })
            .transpose()
    }

    pub fn accept_home(&mut self, event: KeyEvent, revision: u64) -> Result<HomeInput, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        let Some(home) = self.home.as_mut() else {
            return Ok(HomeInput::Unchanged);
        };
        let result = home.accept(event);
        if result != HomeInput::Unchanged {
            self.advance()?;
        }
        Ok(result)
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
    scene
        .push(
            GraphicsCommand::text(
                LayoutRect {
                    x,
                    y,
                    width: screen.width.saturating_sub(x.max(0) as u16 + 24),
                    height: 34,
                },
                screen,
                paint,
                value,
            )
            .and_then(|command| command.with_text_role(role))
            .map_err(|_| Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_human::{KeyModifiers, KeyTransition};

    fn key(usage: u8) -> KeyEvent {
        KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::from_bits(0)).unwrap()
    }

    #[test]
    fn launcher_is_finite_and_opens_prompt() {
        let mut home = Home::new();
        for _ in 0..5 {
            assert_eq!(home.accept(key(43)), HomeInput::Changed);
        }
        assert_eq!(home.accept(key(40)), HomeInput::Changed);
        assert_eq!(home.model.view(), HomeView::Prompt);
    }

    #[test]
    fn command_vocabulary_refuses_unknown_text() {
        let mut model = HomeModel::new();
        assert_eq!(
            model.submit_text("open nowhere", &installed_form_titles()),
            HomeInput::Changed
        );
        assert_eq!(model.output(), "Unknown command: open nowhere");
        assert_eq!(
            model.submit_text("open tour", &installed_form_titles()),
            HomeInput::OpenTour
        );
    }
}
