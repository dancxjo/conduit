//! Finite Forms navigator derived from checked document and U0 navigation truth.

use crate::{gui::GuiAction, PatchbayApplication};
use winit::keyboard::{Key, NamedKey};

pub(super) const VISIBLE_FORM_ROWS: usize = 3;
pub(super) const MAX_BACK_NAVIGATION_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BackNavigationEntry {
    pub(super) parent_form: String,
    pub(super) gear_name: String,
    pub(super) child_form: String,
}

pub(super) enum BackNavigationError {
    TargetMissing,
    TargetUnavailable,
    DepthExceeded,
    Projection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FormNavigatorEntry {
    pub(super) label: String,
    pub(super) action: Option<GuiAction>,
}

impl PatchbayApplication {
    pub(super) fn back_breadcrumb(&self) -> String {
        let Some(editor) = &self.form_editor else {
            return String::new();
        };
        let mut breadcrumb = self
            .back_navigation
            .first()
            .map(|entry| entry.parent_form.clone())
            .unwrap_or_else(|| editor.view().open_form);
        for entry in &self.back_navigation {
            breadcrumb.push_str(" > ");
            breadcrumb.push_str(&entry.gear_name);
            breadcrumb.push_str(" : ");
            breadcrumb.push_str(&entry.child_form);
        }
        breadcrumb
    }

    pub(super) fn open_selected_back(&mut self) -> Result<(), BackNavigationError> {
        let selected = std::mem::take(&mut self.pending_back_selection);
        let target = self
            .pending_back_target
            .take()
            .or_else(|| self.selected_back_target());
        self.apply_back_navigation(target, selected)
    }

    pub(super) fn selected_back_target(&self) -> Option<BackNavigationEntry> {
        let current_form = self.form_editor.as_ref()?.view().open_form;
        self.selected_graphical_subject().and_then(|subject| {
            self.graphical_form.as_ref().and_then(|graph| {
                if let Some(composition) = graph
                    .compositions
                    .iter()
                    .find(|composition| composition.identity == subject.subject_identity)
                {
                    return Some(BackNavigationEntry {
                        parent_form: current_form.clone(),
                        gear_name: composition.gear_name.clone(),
                        child_form: composition.back_name.clone(),
                    });
                }
                graph
                    .gears
                    .iter()
                    .find(|gear| gear.identity == subject.subject_identity)
                    .filter(|gear| gear.source_form != current_form)
                    .map(|gear| {
                        let gear_name = gear
                            .form_path
                            .iter()
                            .skip_while(|segment| *segment != &current_form)
                            .nth(1)
                            .cloned()
                            .unwrap_or_else(|| gear.gear_id.as_str().to_owned());
                        BackNavigationEntry {
                            parent_form: current_form.clone(),
                            gear_name,
                            child_form: gear.source_form.clone(),
                        }
                    })
            })
        })
    }

    fn apply_back_navigation(
        &mut self,
        target: Option<BackNavigationEntry>,
        selected: bool,
    ) -> Result<(), BackNavigationError> {
        if let Some(target) = target {
            if self.back_navigation.len() == MAX_BACK_NAVIGATION_DEPTH {
                return Err(BackNavigationError::DepthExceeded);
            }
            self.form_editor
                .as_mut()
                .expect("editor presence was checked")
                .open_back(&target.child_form)
                .map_err(|_| BackNavigationError::Projection)?;
            self.back_navigation.push(target);
        } else if let Some(target) = self.back_navigation.pop() {
            self.form_editor
                .as_mut()
                .expect("editor presence was checked")
                .open_back(&target.parent_form)
                .map_err(|_| BackNavigationError::Projection)?;
        } else {
            return Err(if selected {
                BackNavigationError::TargetUnavailable
            } else {
                BackNavigationError::TargetMissing
            });
        }
        self.form_selection = 0;
        self.refresh_graphical_form()
            .map_err(|_| BackNavigationError::Projection)?;
        Ok(())
    }

    pub(super) fn form_navigator_entries(&self) -> Vec<FormNavigatorEntry> {
        let (Some(editor), Some(graph)) = (&self.form_editor, &self.graphical_form) else {
            return Vec::new();
        };
        let view = editor.view();
        let mut entries = Vec::with_capacity(view.checked.forms.len());
        let current_is_root = self.back_navigation.is_empty();
        entries.push(FormNavigatorEntry {
            label: if current_is_root {
                format!("ROOT {} [CURRENT]", view.open_form)
            } else {
                format!("OPEN BACK {} [CURRENT]", view.open_form)
            },
            action: None,
        });
        for (index, ancestor) in self.back_navigation.iter().enumerate().rev() {
            entries.push(FormNavigatorEntry {
                label: if index == 0 {
                    format!("ROOT {} [ANCESTOR]", ancestor.parent_form)
                } else {
                    format!("ANCESTOR {}", ancestor.parent_form)
                },
                action: Some(GuiAction::OpenNavigatorAncestor {
                    source_document_id: graph.source_document_id.as_str().into(),
                    checked_form_id: graph.checked_form_id.as_str().into(),
                    expanded_form_id: graph.expanded_form_id.as_str().into(),
                    back_count: self.back_navigation.len() - index,
                }),
            });
        }
        for composition in &graph.compositions {
            let action = graph
                .subject_ref(&composition.identity)
                .ok()
                .map(GuiAction::OpenNavigatorComposition);
            entries.push(FormNavigatorEntry {
                label: format!(
                    "CHILD {} : {}",
                    composition.gear_name, composition.back_name
                ),
                action,
            });
        }
        for form in &view.checked.forms {
            let represented = form.name == view.open_form
                || self
                    .back_navigation
                    .iter()
                    .any(|entry| entry.parent_form == form.name)
                || graph
                    .compositions
                    .iter()
                    .any(|child| child.back_name == form.name);
            if !represented {
                entries.push(FormNavigatorEntry {
                    label: format!("FORM {} [UNAVAILABLE / NO PATH]", form.name),
                    action: None,
                });
            }
        }
        entries
    }

    pub(super) fn handle_navigator_key(&mut self, key: &Key) -> Result<bool, String> {
        if !self.modifiers.alt_key() || self.linear_view {
            return Ok(false);
        }
        let entries = self.form_navigator_entries();
        if entries.is_empty() {
            return Ok(false);
        }
        match key {
            Key::Named(NamedKey::ArrowUp) => {
                self.navigator_selection = self.navigator_selection.saturating_sub(1);
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.navigator_selection = self
                    .navigator_selection
                    .saturating_add(1)
                    .min(entries.len() - 1);
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(action) = entries[self.navigator_selection].action.clone() {
                    self.handle_gui_action(action)?;
                } else {
                    self.publish_refusal("Navigator row is explicitly unavailable");
                }
                return Ok(true);
            }
            _ => return Ok(false),
        }
        self.navigator_scroll = self.navigator_scroll.min(self.navigator_selection);
        if self.navigator_selection >= self.navigator_scroll + VISIBLE_FORM_ROWS {
            self.navigator_scroll = self.navigator_selection + 1 - VISIBLE_FORM_ROWS;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(true)
    }

    pub(super) fn open_navigator_composition(
        &mut self,
        subject: patchbay_model::PatchbaySubjectRef,
    ) -> Result<(), String> {
        self.dispatch_selection(subject.clone())?;
        if self.selected_graphical_subject().as_ref() != Some(&subject) {
            return Ok(());
        }
        self.dispatch_invocation(patchbay_model::PatchbayAction::OpenBack)
    }

    pub(super) fn open_navigator_ancestor(
        &mut self,
        source_document_id: &str,
        checked_form_id: &str,
        expanded_form_id: &str,
        back_count: usize,
    ) -> Result<(), String> {
        let graph = self
            .graphical_form
            .as_ref()
            .ok_or("graphical Form projection is absent")?;
        if graph.source_document_id.as_str() != source_document_id
            || graph.checked_form_id.as_str() != checked_form_id
            || graph.expanded_form_id.as_str() != expanded_form_id
        {
            return Err("stale Forms navigator identity".into());
        }
        if back_count == 0 || back_count > self.back_navigation.len() {
            return Err("stale Forms navigator ancestry".into());
        }
        for _ in 0..back_count {
            self.dispatch_invocation(patchbay_model::PatchbayAction::OpenBack)?;
        }
        Ok(())
    }
}
