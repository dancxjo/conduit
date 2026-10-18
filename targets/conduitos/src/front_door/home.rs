//! Finite ConduitOS Home and command-bar face state.

use alloc::{format, string::String, vec::Vec};
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};
pub use conduit_home_model::{HomeAction as HomeInput, HomeView};
use conduit_home_model::{HomeEvent, HomeModel};
use conduit_human::{KeyEvent, KeyTransition, PreparedKeymap};

use super::{Error, FrontDoor};

pub(super) struct Home {
    model: HomeModel,
    keymap: PreparedKeymap,
    refusal: Option<String>,
}

impl Home {
    fn new() -> Self {
        Self {
            model: HomeModel::new(),
            keymap: PreparedKeymap::default_layout(),
            refusal: None,
        }
    }

    fn accept(&mut self, event: KeyEvent, plots: &[&str]) -> HomeInput {
        if event.transition() != KeyTransition::Pressed {
            let _ = self.keymap.apply(event);
            return HomeInput::Unchanged;
        }
        let abstract_event = match event.usage() {
            79 | 81 | 43 => HomeEvent::Next,
            80 | 82 => HomeEvent::Previous,
            40 => HomeEvent::Activate,
            41 => HomeEvent::Escape,
            42 => HomeEvent::Backspace,
            _ => match self.keymap.apply(event) {
                Ok(Some(fragment)) => {
                    let text = fragment.as_slice();
                    return core::str::from_utf8(text).map_or(HomeInput::Unchanged, |text| {
                        self.model.accept(HomeEvent::Text(text), plots)
                    });
                }
                Err(_) => HomeEvent::CharacterUnavailable,
                _ => return HomeInput::Unchanged,
            },
        };
        self.model.accept(abstract_event, plots)
    }
}

impl FrontDoor {
    /// Exact observed Body membership, distinct from the Host's available catalog.
    pub fn home_installed_plots(&self) -> &[crate::product_journey::WorkspacePlot] {
        self.workspace
            .as_ref()
            .map_or(&[], |workspace| workspace.plots.as_slice())
    }

    fn home_installed_titles(&self) -> Vec<&'static str> {
        self.home_installed_plots()
            .iter()
            .map(|plot| plot.title)
            .collect()
    }

    pub(super) fn home_refusal(&self) -> Option<&str> {
        self.home.as_ref().and_then(|home| home.refusal.as_deref())
    }

    pub(super) fn home_model(&self) -> Option<&HomeModel> {
        self.home.as_ref().map(|home| &home.model)
    }

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
            journey.friendly_name.as_deref().unwrap_or("Current body"),
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
            HomeView::Plots => {
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    128,
                    "INSTALLED PLOTS",
                    GraphicsTextRole::Heading,
                    GraphicsPaintRole::Accent,
                )?;
                for (index, plot) in self.home_installed_plots().iter().enumerate() {
                    draw_text(
                        &mut scene,
                        screen,
                        72,
                        188 + index as i16 * 48,
                        &format!(
                            "{}  {}",
                            if index == home.model.selected_plot_index() {
                                ">"
                            } else {
                                " "
                            },
                            plot.title
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
                        journey.friendly_name.as_deref().unwrap_or("Current body")
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
        if let Some(reason) = &home.refusal {
            draw_text(
                &mut scene,
                screen,
                34,
                i16::try_from(screen.height.saturating_sub(104)).map_err(|_| Error::Scene)?,
                reason,
                GraphicsTextRole::Status,
                GraphicsPaintRole::Status,
            )?;
        }
        let status_y = i16::try_from(screen.height.saturating_sub(52)).map_err(|_| Error::Scene)?;
        if let Some(refusal) = &self.refusal {
            draw_text(
                &mut scene,
                screen,
                24,
                status_y - 38,
                &format!("{} {}", refusal.heading(), refusal.reason()),
                GraphicsTextRole::Warning,
                GraphicsPaintRole::Status,
            )?;
        }
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
            return Err(Error::Face);
        }
        self.home = Some(Home::new());
        self.advance()
    }

    pub fn home_open(&self) -> bool {
        self.home.is_some()
    }

    pub fn close_home(&mut self) -> Result<(), Error> {
        if self.home.take().is_none() {
            return Err(Error::Face);
        }
        self.advance()
    }

    pub fn home_view(&self) -> Option<HomeView> {
        self.home.as_ref().map(|home| home.model.view())
    }

    pub fn home_selection(&self) -> Option<usize> {
        self.home.as_ref().map(|home| home.model.selected_index())
    }

    pub fn home_application_view(&self) -> Result<Option<conduit_face::ApplicationView>, Error> {
        self.home
            .as_ref()
            .map(|home| {
                home.model
                    .face(
                        u32::try_from(self.revision).map_err(|_| Error::Face)?,
                        &self.home_installed_titles(),
                    )
                    .lower()
                    .map_err(|_| Error::Face)
            })
            .transpose()
    }

    pub fn accept_home(&mut self, event: KeyEvent, revision: u64) -> Result<HomeInput, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        let titles = self.home_installed_titles();
        let Some(home) = self.home.as_mut() else {
            return Ok(HomeInput::Unchanged);
        };
        let result = home.accept(event, &titles);
        let absent = match result {
            HomeInput::OpenTour
                if !titles.contains(&crate::native_workset::NativePlot::Tour.title()) =>
            {
                Some("Tour")
            }
            HomeInput::OpenPatchbay
                if !titles.contains(&crate::native_workset::NativePlot::Patchbay.title()) =>
            {
                Some("Patchbay")
            }
            _ => None,
        };
        let result = if let Some(title) = absent {
            home.refusal = Some(format!(
                "{title} is unavailable: this Body has no installed {title} Plot."
            ));
            HomeInput::Changed
        } else {
            if result != HomeInput::Unchanged {
                home.refusal = None;
            }
            result
        };
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
            assert_eq!(home.accept(key(43), &[]), HomeInput::Changed);
        }
        assert_eq!(home.accept(key(40), &[]), HomeInput::Changed);
        assert_eq!(home.model.view(), HomeView::Prompt);
    }

    #[test]
    fn command_vocabulary_refuses_unknown_text() {
        let mut model = HomeModel::new();
        assert_eq!(
            model.submit_text("open nowhere", &["Tour"]),
            HomeInput::Changed
        );
        assert_eq!(model.output(), "Unknown command: open nowhere");
        assert_eq!(
            model.submit_text("open tour", &["Tour"]),
            HomeInput::OpenTour
        );
    }
}
