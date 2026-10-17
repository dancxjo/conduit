//! Portable editor state. No Plot, Plan, Play or Sign is stored or modified here.
use alloc::{collections::BTreeSet, string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub const PATCHBAY_WORKSPACE_SCHEMA: &str = "conduit.patchbay.workspace/v1";
pub const MAX_WORKSPACE_BYTES: usize = 64 * 1024;
pub const MAX_WORKSPACE_LAYOUTS: usize = 4;
pub const MAX_WORKSPACE_SUBJECTS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceBasis {
    pub source_document_id: String,
    pub checked_plot_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePosition {
    pub subject: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceRouteStyle {
    Straight,
    Curved,
    Orthogonal,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRoute {
    pub subject: String,
    pub style: WorkspaceRouteStyle,
    pub points: Vec<WorkspacePoint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceFrame {
    pub id: String,
    pub title: String,
    pub members: Vec<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceNote {
    pub id: String,
    pub text: String,
    #[serde(deserialize_with = "required_nullable_subject")]
    pub subject: Option<String>,
    pub x: i32,
    pub y: i32,
}

fn required_nullable_subject<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceViewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceLens {
    World,
    Plot,
    Plan,
    Play,
    Signs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLayout {
    pub name: String,
    pub positions: Vec<WorkspacePosition>,
    pub routes: Vec<WorkspaceRoute>,
    pub frames: Vec<WorkspaceFrame>,
    pub notes: Vec<WorkspaceNote>,
    pub collapsed: Vec<String>,
    pub viewport: WorkspaceViewport,
    pub lens: WorkspaceLens,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchbayWorkspace {
    pub schema: String,
    pub basis: WorkspaceBasis,
    pub active_layout: String,
    pub layouts: Vec<WorkspaceLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum WorkspaceError {
    UnsupportedSchema,
    BoundExceeded,
    InvalidIdentity,
    DuplicateIdentity,
    InvalidGeometry,
    InvalidRoute,
    MissingActiveLayout,
    ChangedBasis,
    UnmappedLegacySubject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceCorrelation {
    pub basis_matches: bool,
    /// Retained, never remapped or silently discarded. No layout applies on a changed basis.
    pub orphaned_subjects: Vec<String>,
}

impl PatchbayWorkspace {
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.schema != PATCHBAY_WORKSPACE_SCHEMA {
            return Err(WorkspaceError::UnsupportedSchema);
        }
        identity(&self.basis.source_document_id)?;
        identity(&self.basis.checked_plot_id)?;
        text(&self.active_layout, 64, false)?;
        if self.layouts.is_empty() || self.layouts.len() > MAX_WORKSPACE_LAYOUTS {
            return Err(WorkspaceError::BoundExceeded);
        }
        unique(self.layouts.iter().map(|layout| layout.name.as_str()))?;
        for layout in &self.layouts {
            layout.validate()?;
        }
        if !self
            .layouts
            .iter()
            .any(|layout| layout.name == self.active_layout)
        {
            return Err(WorkspaceError::MissingActiveLayout);
        }
        Ok(())
    }

    pub fn correlate(
        &self,
        basis: &WorkspaceBasis,
        subjects: &[String],
    ) -> Result<WorkspaceCorrelation, WorkspaceError> {
        self.validate()?;
        let available: BTreeSet<_> = subjects.iter().map(String::as_str).collect();
        let mut orphaned = BTreeSet::new();
        for layout in &self.layouts {
            for subject in layout
                .positions
                .iter()
                .map(|item| &item.subject)
                .chain(layout.routes.iter().map(|item| &item.subject))
                .chain(layout.frames.iter().flat_map(|item| item.members.iter()))
                .chain(layout.notes.iter().filter_map(|item| item.subject.as_ref()))
                .chain(layout.collapsed.iter())
            {
                if !available.contains(subject.as_str()) {
                    orphaned.insert(subject.clone());
                }
            }
        }
        Ok(WorkspaceCorrelation {
            basis_matches: &self.basis == basis,
            orphaned_subjects: orphaned.into_iter().collect(),
        })
    }

    pub fn layout_for_basis(
        &self,
        basis: &WorkspaceBasis,
    ) -> Result<&WorkspaceLayout, WorkspaceError> {
        self.validate()?;
        if &self.basis != basis {
            return Err(WorkspaceError::ChangedBasis);
        }
        self.layouts
            .iter()
            .find(|layout| layout.name == self.active_layout)
            .ok_or(WorkspaceError::MissingActiveLayout)
    }
}

impl WorkspaceLayout {
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        text(&self.name, 64, false)?;
        if self.positions.len() > MAX_WORKSPACE_SUBJECTS
            || self.routes.len() > MAX_WORKSPACE_SUBJECTS
            || self.frames.len() > 32
            || self.notes.len() > 64
            || self.collapsed.len() > MAX_WORKSPACE_SUBJECTS
        {
            return Err(WorkspaceError::BoundExceeded);
        }
        unique(self.positions.iter().map(|item| item.subject.as_str()))?;
        unique(self.routes.iter().map(|item| item.subject.as_str()))?;
        unique(self.frames.iter().map(|item| item.id.as_str()))?;
        unique(self.notes.iter().map(|item| item.id.as_str()))?;
        unique(self.collapsed.iter().map(String::as_str))?;
        for item in &self.positions {
            point(item.x, item.y)?;
        }
        for item in &self.routes {
            if item.points.len() > 32 {
                return Err(WorkspaceError::BoundExceeded);
            }
            if (item.style == WorkspaceRouteStyle::Manual) != !item.points.is_empty() {
                return Err(WorkspaceError::InvalidRoute);
            }
            for p in &item.points {
                point(p.x, p.y)?;
            }
        }
        for item in &self.frames {
            text(&item.title, 256, false)?;
            point(item.x, item.y)?;
            if item.width < 1 || item.height < 1 {
                return Err(WorkspaceError::InvalidGeometry);
            }
            point(item.width, item.height)?;
            if item.members.len() > MAX_WORKSPACE_SUBJECTS {
                return Err(WorkspaceError::BoundExceeded);
            }
            unique(item.members.iter().map(String::as_str))?;
        }
        for item in &self.notes {
            text(&item.text, 2048, false)?;
            if let Some(subject) = &item.subject {
                identity(subject)?;
            }
            point(item.x, item.y)?;
        }
        self.viewport.validate()
    }
}

impl WorkspaceViewport {
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if !self.x.is_finite()
            || !self.y.is_finite()
            || !self.zoom.is_finite()
            || self.x.abs() > 32767.0
            || self.y.abs() > 32767.0
            || !(0.2..=3.0).contains(&self.zoom)
        {
            return Err(WorkspaceError::InvalidGeometry);
        }
        Ok(())
    }
}

pub(crate) fn point(x: i32, y: i32) -> Result<(), WorkspaceError> {
    if x.unsigned_abs() > 32767 || y.unsigned_abs() > 32767 {
        return Err(WorkspaceError::InvalidGeometry);
    }
    Ok(())
}

fn identity(value: &str) -> Result<(), WorkspaceError> {
    text(value, 512, false)
}

fn text(value: &str, max: usize, empty: bool) -> Result<(), WorkspaceError> {
    if value.len() > max {
        return Err(WorkspaceError::BoundExceeded);
    }
    if (!empty && value.is_empty()) || value.chars().any(char::is_control) {
        return Err(WorkspaceError::InvalidIdentity);
    }
    Ok(())
}

fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Result<(), WorkspaceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        identity(value)?;
        if !seen.insert(value) {
            return Err(WorkspaceError::DuplicateIdentity);
        }
    }
    Ok(())
}
