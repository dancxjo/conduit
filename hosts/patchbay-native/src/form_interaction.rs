//! Canonical Form editing and renderer-local graph selection interaction.

use super::{
    file_task::DestinationPolicy, gui::GuiAction, resource::save_form_resource, PatchbayApplication,
};
use patchbay_model::FormEditor;
use winit::keyboard::{Key, NamedKey};

pub(super) fn graphical_form_for_editor(
    editor: &FormEditor,
) -> Result<Option<patchbay_model::PatchbayGraph>, String> {
    let view = editor.view();
    if !view.checked.diagnostics.is_empty()
        || !view
            .checked
            .forms
            .iter()
            .any(|form| form.name == view.open_form)
    {
        return Ok(None);
    }
    let open_form = view
        .checked
        .forms
        .iter()
        .find(|form| form.name == view.open_form)
        .expect("open Form presence was checked");
    if !open_form.face.inputs().is_empty() || !open_form.face.outputs().is_empty() {
        return Ok(None);
    }
    let expanded = editor
        .expand_form(&view.open_form)
        .map_err(|error| error.to_string())?;
    patchbay_model::PatchbayGraph::from_expanded(&expanded)
        .map(Some)
        .map_err(|error| error.to_string())
}

impl PatchbayApplication {
    pub(super) fn handle_gui_action(&mut self, action: GuiAction) -> Result<(), String> {
        match action {
            GuiAction::SelectSubject(subject) => {
                let graph = self
                    .graphical_form
                    .as_ref()
                    .ok_or("graphical Form projection is absent")?;
                self.graphical_selection = graph
                    .resolve_subject_ref(&subject)
                    .map_err(|error| error.to_string())?;
            }
            GuiAction::OpenNextForm => self.open_next_form()?,
            GuiAction::SaveForm => save_form_resource(
                self.form_editor
                    .as_mut()
                    .ok_or("canonical Form editor is absent")?,
            )?,
            GuiAction::ToggleLinearView => self.linear_view = !self.linear_view,
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    fn open_next_form(&mut self) -> Result<(), String> {
        let editor = self
            .form_editor
            .as_mut()
            .ok_or("canonical Form editor is absent")?;
        let view = editor.view();
        if !view.checked.forms.is_empty() {
            let current = view
                .checked
                .forms
                .iter()
                .position(|form| form.name == view.open_form)
                .unwrap_or(0);
            let next = &view.checked.forms[(current + 1) % view.checked.forms.len()].name;
            editor.open_back(next).map_err(|error| error.to_string())?;
            self.form_selection = 0;
            self.refresh_graphical_form()?;
        }
        Ok(())
    }

    pub(super) fn edit_source(&mut self, update: impl FnOnce(&mut String)) -> Result<(), String> {
        let editor = self
            .form_editor
            .as_mut()
            .ok_or("canonical Form editor is absent")?;
        let mut source = editor.view().source;
        update(&mut source);
        editor
            .replace_source(source)
            .map_err(|error| error.to_string())?;
        editor.recheck().map_err(|error| error.to_string())?;
        self.form_selection = 0;
        self.refresh_graphical_form()?;
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
            window.request_redraw();
        }
        Ok(())
    }

    fn refresh_graphical_form(&mut self) -> Result<(), String> {
        self.graphical_form = self
            .form_editor
            .as_ref()
            .map(graphical_form_for_editor)
            .transpose()?
            .flatten();
        self.graphical_selection = 0;
        Ok(())
    }

    pub(super) fn selected_graphical_identity(&self) -> Option<&str> {
        self.graphical_form
            .as_ref()?
            .subject_identities()
            .nth(self.graphical_selection)
    }

    fn move_graphical_selection(&mut self, forward: bool) {
        let count = self
            .graphical_form
            .as_ref()
            .map_or(0, |graph| graph.subject_identities().count());
        if count == 0 {
            return;
        }
        self.graphical_selection = if forward {
            (self.graphical_selection + 1) % count
        } else {
            (self.graphical_selection + count - 1) % count
        };
    }

    pub(super) fn handle_form_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.form_editor.is_none() {
            return Ok(false);
        }
        match key {
            Key::Named(NamedKey::Backspace) => self.edit_source(|source| {
                source.pop();
            })?,
            Key::Named(NamedKey::Enter) => self.edit_source(|source| source.push('\n'))?,
            Key::Named(NamedKey::Tab) => self.open_next_form()?,
            Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowRight)
                if self.graphical_form.is_some() && !self.linear_view =>
            {
                self.move_graphical_selection(true);
            }
            Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowLeft)
                if self.graphical_form.is_some() && !self.linear_view =>
            {
                self.move_graphical_selection(false);
            }
            Key::Named(NamedKey::ArrowDown) => {
                let editor = self
                    .form_editor
                    .as_ref()
                    .expect("editor presence was checked");
                let view = editor.view();
                let count = editor
                    .view()
                    .checked
                    .forms
                    .iter()
                    .find(|form| form.name == view.open_form)
                    .map(|form| form.items.len())
                    .unwrap_or(0);
                if count > 0 {
                    self.form_selection = (self.form_selection + 1) % count;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.form_selection = self.form_selection.saturating_sub(1)
            }
            Key::Named(NamedKey::F2) => self.linear_view = !self.linear_view,
            Key::Named(NamedKey::F4) => self.birth_body()?,
            Key::Named(NamedKey::F5) => self.wake_body()?,
            Key::Named(NamedKey::F6) => self.plan_play()?,
            Key::Named(NamedKey::F7) if !self.modifiers.alt_key() => self.play_plan()?,
            Key::Named(NamedKey::F8) if !self.modifiers.alt_key() => self.mark_unsatisfied()?,
            Key::Named(NamedKey::F9) if !self.modifiers.alt_key() => self.lull_body()?,
            Key::Named(NamedKey::Escape) => self.control.stop()?,
            Key::Named(NamedKey::F7) => {
                self.file_task.choose_source()?;
            }
            Key::Named(NamedKey::F8) => {
                let policy = if self.modifiers.shift_key() {
                    DestinationPolicy::Replace
                } else {
                    DestinationPolicy::Create
                };
                self.file_task.choose_destination(policy)?;
            }
            Key::Named(NamedKey::F9) => self.file_task.plan()?,
            Key::Named(NamedKey::F10) => self.file_task.run()?,
            Key::Named(NamedKey::F11) => self.file_task.stop()?,
            Key::Character(character)
                if self.modifiers.control_key() && character.eq_ignore_ascii_case("s") =>
            {
                save_form_resource(
                    self.form_editor
                        .as_mut()
                        .expect("editor presence was checked"),
                )?;
            }
            Key::Character(character)
                if !self.modifiers.control_key() && !self.modifiers.super_key() =>
            {
                let characters = character.clone();
                self.edit_source(|source| source.push_str(&characters))?;
            }
            _ => return Ok(false),
        }
        let editor = self
            .form_editor
            .as_mut()
            .expect("editor presence was checked");
        let view = editor.view();
        if let Some(identity) = view
            .checked
            .forms
            .iter()
            .find(|form| form.name == view.open_form)
            .and_then(|form| form.items.get(self.form_selection))
            .map(|item| item.identity.clone())
        {
            editor.select_graph_item(&identity);
        }
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
            window.request_redraw();
        }
        Ok(true)
    }
}
