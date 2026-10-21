//! Finite read-only Home values; no mutable Root HomeModel.
use crate::wire::Refusal;
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u32,
    pub view: u8,
    pub selected: u8,
    pub plot_selected: u8,
    pub command: String,
    pub output: String,
    pub refusal: Option<String>,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.revision == 0
            || self.view > 3
            || self.selected >= 6
            || self.plot_selected >= 16
            || self.command.len() > 96
            || self.output.len() > 2048
            || self.refusal.as_ref().is_some_and(|s| s.len() > 256)
        {
            return Err(Refusal::Malformed);
        }
        Ok(())
    }
    pub fn view(&self) -> conduit_home_model::HomeView {
        match self.view {
            0 => conduit_home_model::HomeView::Launcher,
            1 => conduit_home_model::HomeView::Plots,
            2 => conduit_home_model::HomeView::Body,
            _ => conduit_home_model::HomeView::Prompt,
        }
    }
    pub fn selected_index(&self) -> usize {
        self.selected as usize
    }
    pub fn selected_plot_index(&self) -> usize {
        self.plot_selected as usize
    }
    pub fn selected_destination(&self) -> conduit_home_model::HomeDestination {
        conduit_home_model::HomeDestination::ALL
            .get(self.selected_index())
            .copied()
            .unwrap_or(conduit_home_model::HomeDestination::Tour)
    }
    pub fn command(&self) -> &str {
        &self.command
    }
    pub fn output(&self) -> &str {
        &self.output
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub snapshot: Snapshot,
    pub action: u8,
    pub index: u32,
}
impl Reply {
    pub fn action(&self, plots: usize) -> Result<conduit_home_model::HomeAction, Refusal> {
        use conduit_home_model::HomeAction::*;
        self.snapshot.validate()?;
        if self.action < 5 && self.index != 0 {
            return Err(Refusal::Malformed);
        }
        Ok(match self.action {
            0 => Unchanged,
            1 => Changed,
            2 => OpenTour,
            3 => OpenPatchbay,
            4 => OpenCreche,
            5 if (self.index as usize) < plots => OpenPlot(self.index as usize),
            6 if (self.index as usize) < plots => RunPlot(self.index as usize),
            _ => return Err(Refusal::Malformed),
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub revision: u32,
    pub key: [u8; 3],
    pub titles: Vec<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub revision: u32,
    pub face_revision: u32,
    pub titles: Vec<String>,
}
pub fn validate_titles(titles: &[String]) -> Result<(), Refusal> {
    if titles.len() > 16
        || titles.iter().enumerate().any(|(i, title)| {
            title.is_empty()
                || title.len() > 64
                || title.chars().any(char::is_control)
                || titles[..i].contains(title)
        })
    {
        return Err(Refusal::Bounds);
    }
    Ok(())
}
