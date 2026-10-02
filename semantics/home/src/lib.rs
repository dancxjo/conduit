//! Resident Home presentation meaning shared by ConduitOS realizations.

#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::HomeDestination;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, StatusKind,
};

pub const MAX_COMMAND_BYTES: usize = 96;
pub const HOME_ITEM_COUNT: usize = 6;

pub const HOME_ARRIVED_STEP_ID: &str = "home.arrived";
pub const PLOTS_OPENED_STEP_ID: &str = "plots.opened";
pub const PLOT_SELECTED_STEP_ID: &str = "plot.selected";
pub const PROMPT_OPENED_STEP_ID: &str = "prompt.opened";
pub const PLOT_RUN_STEP_ID: &str = "plot.run";
pub const PLAY_OBSERVED_STEP_ID: &str = "play.observed";
pub const PATCHBAY_REQUESTED_STEP_ID: &str = "patchbay.requested";
pub const HOME_RETURNED_STEP_ID: &str = "home.returned";
pub const OPEN_TOUR_ACTION_ID: &str = "home.open-tour";
pub const OPEN_PATCHBAY_ACTION_ID: &str = "home.open-patchbay";
pub const OPEN_PLOTS_ACTION_ID: &str = "home.open-plots";
pub const OPEN_BODY_ACTION_ID: &str = "home.open-body";
pub const OPEN_CRECHE_ACTION_ID: &str = "home.open-creche";
pub const OPEN_PROMPT_ACTION_ID: &str = "home.open-prompt";

pub const JOURNEY_STEP_IDS: [&str; 8] = [
    HOME_ARRIVED_STEP_ID,
    PLOTS_OPENED_STEP_ID,
    PLOT_SELECTED_STEP_ID,
    PROMPT_OPENED_STEP_ID,
    PLOT_RUN_STEP_ID,
    PLAY_OBSERVED_STEP_ID,
    PATCHBAY_REQUESTED_STEP_ID,
    HOME_RETURNED_STEP_ID,
];

impl HomeDestination {
    pub const ALL: [Self; HOME_ITEM_COUNT] = [
        Self::Tour,
        Self::Patchbay,
        Self::Plots,
        Self::Body,
        Self::Creche,
        Self::Prompt,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Tour => "TOUR",
            Self::Patchbay => "PATCHBAY",
            Self::Plots => "PLOTS",
            Self::Body => "BODY",
            Self::Creche => "CRECHE",
            Self::Prompt => "PROMPT",
        }
    }

    pub const fn action_id(self) -> &'static str {
        match self {
            Self::Tour => OPEN_TOUR_ACTION_ID,
            Self::Patchbay => OPEN_PATCHBAY_ACTION_ID,
            Self::Plots => OPEN_PLOTS_ACTION_ID,
            Self::Body => OPEN_BODY_ACTION_ID,
            Self::Creche => OPEN_CRECHE_ACTION_ID,
            Self::Prompt => OPEN_PROMPT_ACTION_ID,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeView {
    Launcher,
    Plots,
    Body,
    Prompt,
}

impl HomeView {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launcher => "launcher",
            Self::Plots => "plots",
            Self::Body => "body",
            Self::Prompt => "prompt",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeEvent<'a> {
    Next,
    Previous,
    Activate,
    Escape,
    Backspace,
    Submit,
    Text(&'a str),
    CharacterUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HomeAction {
    Unchanged,
    Changed,
    OpenTour,
    OpenPatchbay,
    OpenCreche,
    OpenPlot(usize),
    RunPlot(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeModel {
    selected: usize,
    plot_selected: usize,
    view: HomeView,
    command: String,
    output: String,
}

impl Default for HomeModel {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeModel {
    pub fn new() -> Self {
        Self {
            selected: 0,
            plot_selected: 0,
            view: HomeView::Launcher,
            command: String::new(),
            output: "Type help, or choose a place to begin.".into(),
        }
    }

    pub const fn view(&self) -> HomeView {
        self.view
    }

    pub const fn selected_index(&self) -> usize {
        self.selected
    }

    pub const fn selected_destination(&self) -> HomeDestination {
        HomeDestination::ALL[self.selected]
    }

    pub const fn selected_plot_index(&self) -> usize {
        self.plot_selected
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn presentation(&self, revision: u32, installed_plots: &[&str]) -> SemanticApplicationView {
        let content = match self.view {
            HomeView::Launcher => node(
                "launcher",
                PresentationMechanism::Panel {
                    title: "Conduit Home".into(),
                },
                vec![node(
                    "applications",
                    PresentationMechanism::Grid,
                    HomeDestination::ALL
                        .iter()
                        .enumerate()
                        .map(|(index, destination)| {
                            action_node(
                                &format!("application-{index}"),
                                destination.action_id(),
                                destination.label(),
                            )
                        })
                        .collect(),
                )],
            ),
            HomeView::Plots => node(
                "plots",
                PresentationMechanism::Panel {
                    title: "Installed plots".into(),
                },
                vec![node(
                    "plot-list",
                    PresentationMechanism::ActionGroup {
                        label: "Installed plots".into(),
                    },
                    installed_plots
                        .iter()
                        .enumerate()
                        .map(|(index, title)| {
                            action_node(
                                &format!("plot-{index}"),
                                &format!("home.open-plot.{index}"),
                                title,
                            )
                        })
                        .collect(),
                )],
            ),
            HomeView::Body => node(
                "body",
                PresentationMechanism::Panel {
                    title: "Body".into(),
                },
                vec![node(
                    "body-guidance",
                    PresentationMechanism::Status {
                        kind: StatusKind::Ordinary,
                        title: "Body status".into(),
                        detail: "Current lifecycle truth is supplied by the presenting Host."
                            .into(),
                    },
                    vec![],
                )],
            ),
            HomeView::Prompt => node(
                "prompt",
                PresentationMechanism::Panel {
                    title: "Conduit Prompt".into(),
                },
                vec![
                    node(
                        "command",
                        PresentationMechanism::CodeBlock {
                            language: "conduit-command".into(),
                            code: self.command.clone(),
                        },
                        vec![],
                    ),
                    node(
                        "command-result",
                        PresentationMechanism::Status {
                            kind: StatusKind::Ordinary,
                            title: "Prompt result".into(),
                            detail: self.output.clone(),
                        },
                        vec![],
                    ),
                ],
            ),
        };
        SemanticApplicationView {
            revision,
            root: node("home", PresentationMechanism::Shell, vec![content]),
        }
    }

    pub fn accept(&mut self, event: HomeEvent<'_>, installed_plots: &[&str]) -> HomeAction {
        if self.view == HomeView::Prompt {
            return self.accept_prompt(event, installed_plots);
        }
        if self.view == HomeView::Plots {
            return self.accept_plots(event, installed_plots.len());
        }
        match event {
            HomeEvent::Next => {
                self.selected = (self.selected + 1) % HOME_ITEM_COUNT;
                HomeAction::Changed
            }
            HomeEvent::Previous => {
                self.selected = (self.selected + HOME_ITEM_COUNT - 1) % HOME_ITEM_COUNT;
                HomeAction::Changed
            }
            HomeEvent::Activate => self.open_selected(),
            HomeEvent::Escape => {
                self.view = HomeView::Launcher;
                HomeAction::Changed
            }
            HomeEvent::Text(fragment) if fragment.bytes().any(|byte| byte.is_ascii_graphic()) => {
                self.view = HomeView::Prompt;
                self.command.clear();
                self.append(fragment);
                HomeAction::Changed
            }
            _ => HomeAction::Unchanged,
        }
    }

    pub fn submit_text(&mut self, command: &str, installed_plots: &[&str]) -> HomeAction {
        self.view = HomeView::Prompt;
        self.command.clear();
        self.append(command);
        self.submit(installed_plots)
    }

    fn accept_plots(&mut self, event: HomeEvent<'_>, plot_count: usize) -> HomeAction {
        match event {
            HomeEvent::Next if plot_count > 0 => {
                self.plot_selected = (self.plot_selected + 1) % plot_count;
                HomeAction::Changed
            }
            HomeEvent::Previous if plot_count > 0 => {
                self.plot_selected = (self.plot_selected + plot_count - 1) % plot_count;
                HomeAction::Changed
            }
            HomeEvent::Activate if plot_count > 0 => HomeAction::OpenPlot(self.plot_selected),
            HomeEvent::Escape => {
                self.view = HomeView::Launcher;
                HomeAction::Changed
            }
            _ => HomeAction::Unchanged,
        }
    }

    fn accept_prompt(&mut self, event: HomeEvent<'_>, installed_plots: &[&str]) -> HomeAction {
        match event {
            HomeEvent::Escape => {
                self.view = HomeView::Launcher;
                self.command.clear();
                HomeAction::Changed
            }
            HomeEvent::Backspace => {
                self.command.pop();
                HomeAction::Changed
            }
            HomeEvent::Submit | HomeEvent::Activate => self.submit(installed_plots),
            HomeEvent::Text(fragment) => {
                self.append(fragment);
                HomeAction::Changed
            }
            HomeEvent::CharacterUnavailable => {
                self.output = "That character is unavailable.".into();
                HomeAction::Changed
            }
            _ => HomeAction::Unchanged,
        }
    }

    fn append(&mut self, fragment: &str) {
        for character in fragment.chars() {
            if character != '\n' && self.command.len() + character.len_utf8() <= MAX_COMMAND_BYTES {
                self.command.push(character);
            }
        }
    }

    fn open_selected(&mut self) -> HomeAction {
        match self.selected_destination() {
            HomeDestination::Tour => HomeAction::OpenTour,
            HomeDestination::Patchbay => HomeAction::OpenPatchbay,
            HomeDestination::Plots => {
                self.view = HomeView::Plots;
                HomeAction::Changed
            }
            HomeDestination::Body => {
                self.view = HomeView::Body;
                HomeAction::Changed
            }
            HomeDestination::Creche => HomeAction::OpenCreche,
            HomeDestination::Prompt => {
                self.view = HomeView::Prompt;
                HomeAction::Changed
            }
        }
    }

    fn submit(&mut self, installed_plots: &[&str]) -> HomeAction {
        let command = self.command.trim().to_ascii_lowercase();
        self.command.clear();
        let (verb, argument) = command.split_once(' ').unwrap_or((&command, ""));
        match (verb, argument.trim()) {
            ("help", "") => {
                self.output = "home · open tour|patchbay|plots|body|prompt|creche · plots · run <installed plot> · inspect <installed plot>|body · body · hosts/lines/wake (unavailable on this front)".into();
            }
            ("home", "") => {
                self.view = HomeView::Launcher;
                self.output = "Home".into();
            }
            ("plots", "") => self.view = HomeView::Plots,
            ("body", "") => self.view = HomeView::Body,
            ("hosts", "") => {
                self.output = "Host inspection is unavailable on this Home front; open Patchbay for current host truth.".into();
            }
            ("lines", "") => {
                self.output = "Line inspection is unavailable on this Home front; open Patchbay for current Line truth.".into();
            }
            ("open", "tour") => return HomeAction::OpenTour,
            ("open", "patchbay") => return HomeAction::OpenPatchbay,
            ("open", "plots") => self.view = HomeView::Plots,
            ("open", "body") => self.view = HomeView::Body,
            ("open", "prompt") => self.view = HomeView::Prompt,
            ("open", "creche") => return HomeAction::OpenCreche,
            ("run", "") => self.output = "run needs an installed plot name.".into(),
            ("run", requested) => {
                if let Some(index) = resolve_plot(requested, installed_plots) {
                    return HomeAction::RunPlot(index);
                }
                self.output = format!("No installed plot named {requested}.");
            }
            ("inspect", "") => self.output = "inspect needs a visible subject.".into(),
            ("inspect", "body") => {
                self.view = HomeView::Body;
                self.output = "Body summary".into();
            }
            ("inspect", subject) => {
                if let Some(index) = resolve_plot(subject, installed_plots) {
                    return HomeAction::OpenPlot(index);
                }
                self.output = format!("Cannot inspect unresolved subject {subject}.");
            }
            ("wake", "") => {
                self.output =
                    "Wake is unavailable on this Home front; no lifecycle authority is attached."
                        .into();
            }
            ("", "") => {}
            _ => self.output = format!("Unknown command: {command}"),
        }
        HomeAction::Changed
    }
}

fn resolve_plot(requested: &str, installed_plots: &[&str]) -> Option<usize> {
    installed_plots
        .iter()
        .position(|title| title.eq_ignore_ascii_case(requested))
}

fn action_node(key: &str, identity: &str, label: &str) -> SemanticPresentationNode {
    node(
        key,
        PresentationMechanism::Action(SemanticAction {
            identity: identity.into(),
            event: ApplicationEventKind::Activate,
            label: label.into(),
            availability: ActionAvailability::Available,
        }),
        vec![],
    )
}

fn node(
    key: &str,
    mechanism: PresentationMechanism,
    children: Vec<SemanticPresentationNode>,
) -> SemanticPresentationNode {
    SemanticPresentationNode {
        key: key.into(),
        mechanism,
        children,
    }
}

#[cfg(test)]
mod tests;
