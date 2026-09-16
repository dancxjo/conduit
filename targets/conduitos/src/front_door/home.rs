//! Finite ConduitOS Home and command-bar presentation state.

use alloc::{format, string::String};
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyTransition, KeymapDisposition};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, GraphicsTextRole,
    LayoutRect,
};

use super::{Error, FrontDoor};

const HOME_ITEM_COUNT: usize = 6;
const MAX_COMMAND_BYTES: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeView {
    Launcher,
    Forms,
    Body,
    Prompt,
}

impl HomeView {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launcher => "launcher",
            Self::Forms => "forms",
            Self::Body => "body",
            Self::Prompt => "prompt",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeInput {
    Unchanged,
    Changed,
    OpenTour,
    OpenPatchbay,
    OpenForm(usize),
    RunForm(usize),
    Wake,
}

pub(super) struct Home {
    selected: usize,
    form_selected: usize,
    view: HomeView,
    command: String,
    output: String,
    keymap: ConduitIntlKeymap,
}

impl Home {
    fn new() -> Self {
        Self {
            selected: 0,
            form_selected: 0,
            view: HomeView::Launcher,
            command: String::new(),
            output: "Type help, or choose a place to begin.".into(),
            keymap: ConduitIntlKeymap::new(),
        }
    }

    fn accept(&mut self, event: KeyEvent) -> HomeInput {
        if event.transition() != KeyTransition::Pressed {
            self.keymap.apply(event);
            return HomeInput::Unchanged;
        }
        if self.view != HomeView::Prompt {
            if self.view == HomeView::Forms {
                return match event.usage() {
                    79 | 81 | 43 => {
                        self.form_selected =
                            (self.form_selected + 1) % crate::native_workset::NATIVE_FORM_CAPACITY;
                        HomeInput::Changed
                    }
                    80 | 82 => {
                        self.form_selected =
                            (self.form_selected + crate::native_workset::NATIVE_FORM_CAPACITY - 1)
                                % crate::native_workset::NATIVE_FORM_CAPACITY;
                        HomeInput::Changed
                    }
                    40 => HomeInput::OpenForm(self.form_selected),
                    41 => {
                        self.view = HomeView::Launcher;
                        HomeInput::Changed
                    }
                    _ => HomeInput::Unchanged,
                };
            }
            return match event.usage() {
                79 | 81 | 43 => {
                    self.selected = (self.selected + 1) % HOME_ITEM_COUNT;
                    HomeInput::Changed
                }
                80 | 82 => {
                    self.selected = (self.selected + HOME_ITEM_COUNT - 1) % HOME_ITEM_COUNT;
                    HomeInput::Changed
                }
                40 => self.open_selected(),
                41 => {
                    self.view = HomeView::Launcher;
                    HomeInput::Changed
                }
                _ => match self.keymap.apply(event) {
                    KeymapDisposition::Text(fragment)
                        if fragment.as_bytes().iter().any(u8::is_ascii_graphic) =>
                    {
                        self.view = HomeView::Prompt;
                        self.command.clear();
                        self.append(fragment.as_bytes());
                        HomeInput::Changed
                    }
                    _ => HomeInput::Unchanged,
                },
            };
        }
        match event.usage() {
            41 => {
                self.view = HomeView::Launcher;
                self.command.clear();
                HomeInput::Changed
            }
            42 => {
                self.command.pop();
                HomeInput::Changed
            }
            40 => self.submit(),
            _ => match self.keymap.apply(event) {
                KeymapDisposition::Text(fragment) => {
                    self.append(fragment.as_bytes());
                    HomeInput::Changed
                }
                KeymapDisposition::Refused(_) => {
                    self.output = "That character is unavailable.".into();
                    HomeInput::Changed
                }
                _ => HomeInput::Unchanged,
            },
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if let Ok(fragment) = core::str::from_utf8(bytes) {
            for character in fragment.chars() {
                if character != '\n'
                    && self.command.len() + character.len_utf8() <= MAX_COMMAND_BYTES
                {
                    self.command.push(character);
                }
            }
        }
    }

    fn open_selected(&mut self) -> HomeInput {
        match self.selected {
            0 => HomeInput::OpenTour,
            1 => HomeInput::OpenPatchbay,
            2 => {
                self.view = HomeView::Forms;
                HomeInput::Changed
            }
            3 => {
                self.view = HomeView::Body;
                HomeInput::Changed
            }
            4 => {
                self.output =
                    "Crèche completed this Body's birth; its biography remains here.".into();
                HomeInput::Changed
            }
            _ => {
                self.view = HomeView::Prompt;
                HomeInput::Changed
            }
        }
    }

    fn submit(&mut self) -> HomeInput {
        let command = self.command.trim().to_ascii_lowercase();
        self.command.clear();
        let (verb, argument) = command.split_once(' ').unwrap_or((&command, ""));
        match (verb, argument.trim()) {
            ("help", "") => self.output =
                "home  open <place>  forms  run <form>  inspect <thing>  body  hosts  lines  wake"
                    .into(),
            ("home", "") => {
                self.view = HomeView::Launcher;
                self.output = "Home".into();
            }
            ("forms", "") => self.view = HomeView::Forms,
            ("body", "") | ("hosts", "") | ("lines", "") => self.view = HomeView::Body,
            ("open", "tour") => return HomeInput::OpenTour,
            ("open", "patchbay") => return HomeInput::OpenPatchbay,
            ("open", "forms") => self.view = HomeView::Forms,
            ("open", "body") => self.view = HomeView::Body,
            ("open", "prompt") => {}
            ("open", "creche") => {
                self.output = "Crèche is not reopened over an already-born Body.".into()
            }
            ("run", "") => self.output = "run needs an installed Form name.".into(),
            ("run", requested) => {
                if let Some(index) = crate::native_workset::inventory()
                    .iter()
                    .position(|form| form.title().eq_ignore_ascii_case(requested))
                {
                    return HomeInput::RunForm(index);
                }
                self.output = format!("No installed Form named {requested}.");
            }
            ("inspect", "") => self.output = "inspect needs a visible subject.".into(),
            ("inspect", subject) => self.output = format!("Inspect {subject} from Forms or Body."),
            ("wake", "") => return HomeInput::Wake,
            ("", "") => {}
            _ => self.output = format!("Unknown command: {command}"),
        }
        HomeInput::Changed
    }
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
        match home.view {
            HomeView::Launcher => {
                let items = ["TOUR", "PATCHBAY", "FORMS", "BODY", "CRECHE", "PROMPT"];
                for (index, label) in items.iter().enumerate() {
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
                                if index == home.selected {
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
                        label,
                        GraphicsTextRole::Heading,
                        if index == home.selected {
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
                    &format!("conduct> {}_", home.command),
                    GraphicsTextRole::Code,
                    GraphicsPaintRole::Foreground,
                )?;
                draw_text(
                    &mut scene,
                    screen,
                    56,
                    282,
                    &home.output,
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
                            if index == home.form_selected {
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
        self.home.as_ref().map(|home| home.view)
    }

    pub fn home_selection(&self) -> Option<usize> {
        self.home.as_ref().map(|home| home.selected)
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
        assert_eq!(home.view, HomeView::Prompt);
    }

    #[test]
    fn command_vocabulary_refuses_unknown_text() {
        let mut home = Home::new();
        home.view = HomeView::Prompt;
        home.command = "open nowhere".into();
        assert_eq!(home.submit(), HomeInput::Changed);
        assert_eq!(home.output, "Unknown command: open nowhere");
        home.command = "open tour".into();
        assert_eq!(home.submit(), HomeInput::OpenTour);
    }

    #[test]
    fn run_resolves_only_an_exact_installed_form() {
        let mut home = Home::new();
        home.view = HomeView::Prompt;
        let first = crate::native_workset::inventory()[0].title();
        home.command = format!("run {first}");
        assert_eq!(home.submit(), HomeInput::RunForm(0));

        home.command = "run definitely absent".into();
        assert_eq!(home.submit(), HomeInput::Changed);
        assert_eq!(home.output, "No installed Form named definitely absent.");
    }
}
