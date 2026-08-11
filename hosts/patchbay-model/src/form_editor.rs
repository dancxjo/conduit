//! Revisioned canonical Form source and a presentation-only checked graph.

use conduit_form::{
    check_syntax_document, parse_syntax_document, BackStatement, CheckedCordStage,
    CheckedSyntaxDocument, FormSyntax, Span, StartupCatalog, SyntaxCheckDiagnostic,
};
use std::path::{Path, PathBuf};

pub use crate::form_editor_error::FormEditorError;

const MAX_GRAPH_ITEMS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphItemKind {
    FaceInput,
    FaceOutput,
    StartupValue,
    Gear,
    Cord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphItem {
    pub identity: String,
    pub label: String,
    pub kind: GraphItemKind,
    pub source_span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphForm {
    pub name: String,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub face: conduit_core::CheckedFace,
    pub source_span: Span,
    pub items: Vec<GraphItem>,
    pub cords: Vec<GraphCord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCord {
    pub identity: String,
    pub stages: Vec<GraphCordStage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphCordStage {
    Reference(String),
    InlineGear { kind: String },
    Literal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRevision {
    pub revision: u64,
    pub source_document_id: Option<conduit_core::SourceDocumentId>,
    pub diagnostics: Vec<EditorDiagnostic>,
    pub forms: Vec<GraphForm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSelection {
    pub identity: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormDocumentView {
    pub revision: u64,
    pub saved_revision: u64,
    pub path: PathBuf,
    pub source: String,
    pub checked: CheckedRevision,
    pub open_form: String,
    pub selection: Option<SourceSelection>,
}

pub struct FormEditor {
    pub(crate) path: PathBuf,
    pub(crate) source: String,
    pub(crate) revision: u64,
    pub(crate) saved_revision: u64,
    pub(crate) checked: CheckedRevision,
    pub(crate) open_form: String,
    pub(crate) selection: Option<SourceSelection>,
}

impl FormEditor {
    pub fn from_source(path: PathBuf, source: String) -> Result<Self, FormEditorError> {
        validate_path(&path)?;
        ensure_source_bound(&source)?;
        let checked = check_revision(0, &source)?;
        let open_form = checked
            .forms
            .first()
            .map(|form| form.name.clone())
            .unwrap_or_default();
        Ok(Self {
            path,
            source,
            revision: 0,
            saved_revision: 0,
            checked,
            open_form,
            selection: None,
        })
    }

    pub fn replace_source(&mut self, source: String) -> Result<u64, FormEditorError> {
        ensure_source_bound(&source)?;
        self.revision = self.revision.saturating_add(1);
        self.source = source;
        self.selection = None;
        Ok(self.revision)
    }

    /// Computes a result independently so an async host can publish it later.
    pub fn check_current(&self) -> Result<CheckedRevision, FormEditorError> {
        check_revision(self.revision, &self.source)
    }

    pub fn publish_checked(&mut self, checked: CheckedRevision) -> Result<(), FormEditorError> {
        if checked.revision != self.revision {
            return Err(FormEditorError::StaleRevision {
                current: self.revision,
                offered: checked.revision,
            });
        }
        if !checked.forms.iter().any(|form| form.name == self.open_form) {
            self.open_form = checked
                .forms
                .first()
                .map(|form| form.name.clone())
                .unwrap_or_default();
        }
        self.checked = checked;
        Ok(())
    }

    pub fn recheck(&mut self) -> Result<(), FormEditorError> {
        let checked = self.check_current()?;
        self.publish_checked(checked)
    }

    pub fn mark_saved(&mut self, revision: u64) -> Result<(), FormEditorError> {
        if revision != self.revision {
            return Err(FormEditorError::StaleRevision {
                current: self.revision,
                offered: revision,
            });
        }
        self.saved_revision = revision;
        Ok(())
    }

    pub fn open_back(&mut self, name: &str) -> Result<(), FormEditorError> {
        if !self.checked.forms.iter().any(|form| form.name == name) {
            return Err(FormEditorError::UnknownForm(name.into()));
        }
        self.open_form = name.into();
        self.selection = None;
        Ok(())
    }

    pub fn select_graph_item(&mut self, identity: &str) -> bool {
        let item = self
            .checked
            .forms
            .iter()
            .flat_map(|form| &form.items)
            .find(|item| item.identity == identity);
        self.selection = item.map(|item| SourceSelection {
            identity: item.identity.clone(),
            span: item.source_span,
        });
        self.selection.is_some()
    }

    pub fn select_source_span(&mut self, span: Span) -> bool {
        let item = self
            .checked
            .forms
            .iter()
            .flat_map(|form| &form.items)
            .find(|item| item.source_span == span);
        self.selection = item.map(|item| SourceSelection {
            identity: item.identity.clone(),
            span: item.source_span,
        });
        self.selection.is_some()
    }

    pub fn view(&self) -> FormDocumentView {
        FormDocumentView {
            revision: self.revision,
            saved_revision: self.saved_revision,
            path: self.path.clone(),
            source: self.source.clone(),
            checked: self.checked.clone(),
            open_form: self.open_form.clone(),
            selection: self.selection.clone(),
        }
    }

    pub fn expand_form(
        &self,
        name: &str,
    ) -> Result<conduit_form::ExpandedCanonicalForm, FormEditorError> {
        let syntax = parse_syntax_document(&self.source);
        let (startup, profile) = standard_catalogs()?;
        let checked = check_syntax_document(&syntax, &startup)
            .map_err(|diagnostic| FormEditorError::Catalog(diagnostic.message))?;
        conduit_form::expand_canonical_form(&checked, name, &profile)
            .map_err(|diagnostic| FormEditorError::Catalog(diagnostic.to_string()))
    }
}

pub(crate) fn check_revision(
    revision: u64,
    source: &str,
) -> Result<CheckedRevision, FormEditorError> {
    let syntax = parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Ok(invalid_revision(
            revision,
            diagnostic.code,
            &diagnostic.message,
            diagnostic.span,
        ));
    }
    let (startup, _profile) = standard_catalogs()?;
    match check_syntax_document(&syntax, &startup) {
        Ok(checked) => graph_revision(revision, &syntax.forms, checked),
        Err(diagnostic) => Ok(check_error_revision(revision, diagnostic)),
    }
}

fn standard_catalogs() -> Result<(StartupCatalog, conduit_form::ProfileCatalog), FormEditorError> {
    let mut startup = StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_std_catalog::install_time_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_std_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_std_catalog::install_logic_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    conduit_std_catalog::install_math_catalogs(&mut startup, &mut profile)
        .map_err(FormEditorError::Catalog)?;
    Ok((startup, profile))
}

fn graph_revision(
    revision: u64,
    syntax_forms: &[FormSyntax],
    checked: CheckedSyntaxDocument,
) -> Result<CheckedRevision, FormEditorError> {
    let mut forms = Vec::with_capacity(checked.forms.len());
    for form in &checked.forms {
        let syntax = syntax_forms
            .iter()
            .find(|candidate| candidate.name.text == form.name)
            .expect("checked forms retain parsed names");
        let mut items = Vec::new();
        let mut cords = Vec::new();
        for parameter in &syntax.face.startup_parameters {
            push_item(
                &mut items,
                &form.name,
                "startup",
                &parameter.name.text,
                GraphItemKind::StartupValue,
                parameter.span,
            )?;
        }
        for port in &syntax.face.runtime_ports {
            let kind = match port.direction {
                conduit_form::RuntimePortDirection::Input => GraphItemKind::FaceInput,
                conduit_form::RuntimePortDirection::Output => GraphItemKind::FaceOutput,
            };
            push_item(
                &mut items,
                &form.name,
                "port",
                &port.name.text,
                kind,
                port.span,
            )?;
        }
        let mut cord_index = 0;
        for statement in &syntax.back {
            match statement {
                BackStatement::NamedGear(gear) => {
                    push_item(
                        &mut items,
                        &form.name,
                        "gear",
                        &gear.name.text,
                        GraphItemKind::Gear,
                        gear.span,
                    )?;
                    let operation = form
                        .gears
                        .iter()
                        .find(|checked_gear| checked_gear.name.as_deref() == Some(&gear.name.text))
                        .map(|checked_gear| checked_gear.kind.as_str())
                        .unwrap_or("unknown");
                    items.last_mut().expect("gear item was admitted").label =
                        format!("{}: {operation}", gear.name.text);
                }
                BackStatement::Cord(cord) => {
                    let label = form
                        .cords
                        .get(cord_index)
                        .map(cord_label)
                        .unwrap_or_else(|| "cord".into());
                    push_item(
                        &mut items,
                        &form.name,
                        "cord",
                        &cord_index.to_string(),
                        GraphItemKind::Cord,
                        cord.span,
                    )?;
                    if let Some(item) = items.last_mut() {
                        item.label = label;
                    }
                    if let Some(checked_cord) = form.cords.get(cord_index) {
                        cords.push(GraphCord {
                            identity: items
                                .last()
                                .expect("cord item was admitted")
                                .identity
                                .clone(),
                            stages: checked_cord
                                .stages
                                .iter()
                                .map(|stage| match stage {
                                    CheckedCordStage::Reference(name) => {
                                        GraphCordStage::Reference(name.clone())
                                    }
                                    CheckedCordStage::InlineGear(gear) => {
                                        GraphCordStage::InlineGear {
                                            kind: gear.kind.clone(),
                                        }
                                    }
                                    CheckedCordStage::Literal { .. } => GraphCordStage::Literal,
                                })
                                .collect(),
                        });
                    }
                    cord_index += 1;
                }
                BackStatement::Pool(_) | BackStatement::LocalValue(_) => {}
            }
        }
        forms.push(GraphForm {
            name: form.name.clone(),
            checked_form_id: form.checked_form_id.clone(),
            face: form.checked_face(),
            source_span: syntax.span,
            items,
            cords,
        });
    }
    Ok(CheckedRevision {
        revision,
        source_document_id: Some(checked.source_document_id),
        diagnostics: Vec::new(),
        forms,
    })
}

fn cord_label(cord: &conduit_form::CheckedCanonicalCord) -> String {
    cord.stages
        .iter()
        .map(|stage| match stage {
            CheckedCordStage::Reference(name) => name.clone(),
            CheckedCordStage::InlineGear(gear) => gear.kind.clone(),
            CheckedCordStage::Literal { value, .. } => format!("{value:?}"),
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn push_item(
    items: &mut Vec<GraphItem>,
    form: &str,
    class: &str,
    name: &str,
    kind: GraphItemKind,
    source_span: Span,
) -> Result<(), FormEditorError> {
    if items.len() == MAX_GRAPH_ITEMS {
        return Err(FormEditorError::GraphTooLarge);
    }
    items.push(GraphItem {
        identity: format!("form/{form}/{class}/{name}"),
        label: name.into(),
        kind,
        source_span,
    });
    Ok(())
}

fn check_error_revision(revision: u64, diagnostic: SyntaxCheckDiagnostic) -> CheckedRevision {
    invalid_revision(
        revision,
        diagnostic.code,
        &diagnostic.message,
        diagnostic.span,
    )
}

fn invalid_revision(
    revision: u64,
    code: &'static str,
    message: &str,
    span: Span,
) -> CheckedRevision {
    CheckedRevision {
        revision,
        source_document_id: None,
        diagnostics: vec![EditorDiagnostic {
            code,
            message: message.into(),
            span,
        }],
        forms: Vec::new(),
    }
}

fn validate_path(path: &Path) -> Result<(), FormEditorError> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("conduit") {
        return Err(FormEditorError::NotCanonicalFormPath);
    }
    Ok(())
}

pub(crate) fn ensure_source_bound(source: &str) -> Result<(), FormEditorError> {
    if source.len() > conduit_form::MAXIMUM_FORM_SOURCE_BYTES {
        Err(FormEditorError::SourceTooLarge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "form_editor_tests.rs"]
mod tests;
