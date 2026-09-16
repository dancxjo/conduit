#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_presentation::{
    ActionAvailability, ApplicationEventKind, PresentationMechanism, SemanticAction,
    SemanticApplicationView, SemanticPresentationNode, StatusKind,
};

mod evidence;
mod journey;
mod speech;
pub use evidence::*;
pub use journey::*;
pub use speech::*;

pub const MAX_COMMAND_BYTES: usize = 96;
pub const HOME_ITEM_COUNT: usize = 6;

pub const HOME_ARRIVED_STEP_ID: &str = "home.arrived";
pub const FORMS_OPENED_STEP_ID: &str = "forms.opened";
pub const FORM_SELECTED_STEP_ID: &str = "form.selected";
pub const PROMPT_OPENED_STEP_ID: &str = "prompt.opened";
pub const FORM_RUN_STEP_ID: &str = "form.run";
pub const PLAY_OBSERVED_STEP_ID: &str = "play.observed";
pub const PATCHBAY_OPENED_STEP_ID: &str = "patchbay.opened";
pub const HOME_RETURNED_STEP_ID: &str = "home.returned";
pub const OPEN_TOUR_ACTION_ID: &str = "home.open-tour";
pub const OPEN_PATCHBAY_ACTION_ID: &str = "home.open-patchbay";
pub const OPEN_FORMS_ACTION_ID: &str = "home.open-forms";
pub const OPEN_BODY_ACTION_ID: &str = "home.open-body";
pub const OPEN_CRECHE_ACTION_ID: &str = "home.open-creche";
pub const OPEN_PROMPT_ACTION_ID: &str = "home.open-prompt";

pub const JOURNEY_STEP_IDS: [&str; 8] = [
    HOME_ARRIVED_STEP_ID,
    FORMS_OPENED_STEP_ID,
    FORM_SELECTED_STEP_ID,
    PROMPT_OPENED_STEP_ID,
    FORM_RUN_STEP_ID,
    PLAY_OBSERVED_STEP_ID,
    PATCHBAY_OPENED_STEP_ID,
    HOME_RETURNED_STEP_ID,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HomeDestination {
    Tour,
    Patchbay,
    Forms,
    Body,
    Creche,
    Prompt,
}

impl HomeDestination {
    pub const ALL: [Self; HOME_ITEM_COUNT] = [
        Self::Tour,
        Self::Patchbay,
        Self::Forms,
        Self::Body,
        Self::Creche,
        Self::Prompt,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Tour => "TOUR",
            Self::Patchbay => "PATCHBAY",
            Self::Forms => "FORMS",
            Self::Body => "BODY",
            Self::Creche => "CRECHE",
            Self::Prompt => "PROMPT",
        }
    }

    pub const fn action_id(self) -> &'static str {
        match self {
            Self::Tour => OPEN_TOUR_ACTION_ID,
            Self::Patchbay => OPEN_PATCHBAY_ACTION_ID,
            Self::Forms => OPEN_FORMS_ACTION_ID,
            Self::Body => OPEN_BODY_ACTION_ID,
            Self::Creche => OPEN_CRECHE_ACTION_ID,
            Self::Prompt => OPEN_PROMPT_ACTION_ID,
        }
    }
}

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
    OpenForm(usize),
    RunForm(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomeModel {
    selected: usize,
    form_selected: usize,
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
            form_selected: 0,
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

    pub const fn selected_form_index(&self) -> usize {
        self.form_selected
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn presentation(&self, revision: u32, installed_forms: &[&str]) -> SemanticApplicationView {
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
            HomeView::Forms => node(
                "forms",
                PresentationMechanism::Panel {
                    title: "Installed Forms".into(),
                },
                vec![node(
                    "form-list",
                    PresentationMechanism::ActionGroup {
                        label: "Installed Forms".into(),
                    },
                    installed_forms
                        .iter()
                        .enumerate()
                        .map(|(index, title)| {
                            action_node(
                                &format!("form-{index}"),
                                &format!("home.open-form.{index}"),
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

    pub fn accept(&mut self, event: HomeEvent<'_>, installed_forms: &[&str]) -> HomeAction {
        if self.view == HomeView::Prompt {
            return self.accept_prompt(event, installed_forms);
        }
        if self.view == HomeView::Forms {
            return self.accept_forms(event, installed_forms.len());
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

    pub fn submit_text(&mut self, command: &str, installed_forms: &[&str]) -> HomeAction {
        self.view = HomeView::Prompt;
        self.command.clear();
        self.append(command);
        self.submit(installed_forms)
    }

    fn accept_forms(&mut self, event: HomeEvent<'_>, form_count: usize) -> HomeAction {
        match event {
            HomeEvent::Next if form_count > 0 => {
                self.form_selected = (self.form_selected + 1) % form_count;
                HomeAction::Changed
            }
            HomeEvent::Previous if form_count > 0 => {
                self.form_selected = (self.form_selected + form_count - 1) % form_count;
                HomeAction::Changed
            }
            HomeEvent::Activate if form_count > 0 => HomeAction::OpenForm(self.form_selected),
            HomeEvent::Escape => {
                self.view = HomeView::Launcher;
                HomeAction::Changed
            }
            _ => HomeAction::Unchanged,
        }
    }

    fn accept_prompt(&mut self, event: HomeEvent<'_>, installed_forms: &[&str]) -> HomeAction {
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
            HomeEvent::Submit | HomeEvent::Activate => self.submit(installed_forms),
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
            HomeDestination::Forms => {
                self.view = HomeView::Forms;
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

    fn submit(&mut self, installed_forms: &[&str]) -> HomeAction {
        let command = self.command.trim().to_ascii_lowercase();
        self.command.clear();
        let (verb, argument) = command.split_once(' ').unwrap_or((&command, ""));
        match (verb, argument.trim()) {
            ("help", "") => {
                self.output = "home · open tour|patchbay|forms|body|prompt|creche · forms · run <installed form> · inspect <installed form>|body · body · hosts/lines/wake (unavailable on this face)".into();
            }
            ("home", "") => {
                self.view = HomeView::Launcher;
                self.output = "Home".into();
            }
            ("forms", "") => self.view = HomeView::Forms,
            ("body", "") => self.view = HomeView::Body,
            ("hosts", "") => {
                self.output = "Host inspection is unavailable on this Home face; open Patchbay for current Host truth.".into();
            }
            ("lines", "") => {
                self.output = "Line inspection is unavailable on this Home face; open Patchbay for current Line truth.".into();
            }
            ("open", "tour") => return HomeAction::OpenTour,
            ("open", "patchbay") => return HomeAction::OpenPatchbay,
            ("open", "forms") => self.view = HomeView::Forms,
            ("open", "body") => self.view = HomeView::Body,
            ("open", "prompt") => self.view = HomeView::Prompt,
            ("open", "creche") => return HomeAction::OpenCreche,
            ("run", "") => self.output = "run needs an installed Form name.".into(),
            ("run", requested) => {
                if let Some(index) = resolve_form(requested, installed_forms) {
                    return HomeAction::RunForm(index);
                }
                self.output = format!("No installed Form named {requested}.");
            }
            ("inspect", "") => self.output = "inspect needs a visible subject.".into(),
            ("inspect", "body") => {
                self.view = HomeView::Body;
                self.output = "Body summary".into();
            }
            ("inspect", subject) => {
                if let Some(index) = resolve_form(subject, installed_forms) {
                    return HomeAction::OpenForm(index);
                }
                self.output = format!("Cannot inspect unresolved subject {subject}.");
            }
            ("wake", "") => {
                self.output =
                    "Wake is unavailable on this Home face; no lifecycle authority is attached."
                        .into();
            }
            ("", "") => {}
            _ => self.output = format!("Unknown command: {command}"),
        }
        HomeAction::Changed
    }
}

fn resolve_form(requested: &str, installed_forms: &[&str]) -> Option<usize> {
    installed_forms
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
