//! Canonical Form editing and renderer-local graph selection interaction.

use super::{
    file_task::DestinationPolicy,
    gui::GuiAction,
    resource::{save_form_resource, save_layout_resource},
    PatchbayApplication,
};
use patchbay_model::{
    FormEditor, InteractionDisposition, PatchbayAction, PatchbayInteraction,
    PatchbayInteractionRequest, PatchbayInvocation, PatchbayInvocationOutcome, PatchbayRefusal,
};
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
            GuiAction::SelectSubject(subject) => self.dispatch_selection(subject)?,
            GuiAction::OpenNextForm => self.dispatch_invocation(PatchbayAction::OpenBack)?,
            GuiAction::SaveForm => self.dispatch_invocation(PatchbayAction::Save)?,
            GuiAction::ToggleLinearView => {
                self.dispatch_invocation(PatchbayAction::ToggleLinearView)?
            }
            GuiAction::PlacePaletteKind(kind) => self.dispatch_palette_placement(&kind)?,
            GuiAction::DuplicateGear(subject) => {
                self.dispatch_gear_edit(PatchbayAction::DuplicateGear, &subject)?
            }
            GuiAction::RemoveGear(subject) => {
                self.dispatch_gear_edit(PatchbayAction::RemoveGear, &subject)?
            }
            GuiAction::RemoveCord(subject) => {
                self.dispatch_gear_edit(PatchbayAction::RemoveCord, &subject)?
            }
            GuiAction::ConnectPorts { source, sink } => {
                self.dispatch_port_connection(&source, &sink)?
            }
            GuiAction::RerouteCord { cord, sink } => self.dispatch_cord_reroute(&cord, &sink)?,
            GuiAction::ConfigureGear {
                subject,
                key,
                value,
            } => self.dispatch_gear_configuration(&subject, &key, value)?,
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    fn dispatch_selection(
        &mut self,
        subject: patchbay_model::PatchbaySubjectRef,
    ) -> Result<(), String> {
        let mut interaction = self
            .interaction
            .take()
            .expect("interaction state is installed");
        let graph = self.graphical_form.clone();
        let result = interaction
            .next_request_id("select")
            .and_then(|request_id| PatchbayInteractionRequest::select(request_id, &subject))
            .and_then(|request| {
                interaction.execute(graph.as_ref(), request, |_| {
                    PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable)
                })
            });
        self.interaction = Some(interaction);
        self.finish_interaction(result)
    }

    fn dispatch_invocation(&mut self, action: PatchbayAction) -> Result<(), String> {
        let target = self
            .graphical_form
            .as_ref()
            .map(|graph| graph.expanded_form_id.as_str().to_owned())
            .or_else(|| {
                self.form_editor
                    .as_ref()
                    .map(|editor| editor.view().open_form)
            })
            .ok_or("canonical Form target is absent")?;
        let mut interaction = self
            .interaction
            .take()
            .expect("interaction state is installed");
        let graph = self.graphical_form.clone();
        let result = interaction
            .next_request_id(action.as_str())
            .and_then(|request_id| PatchbayInteractionRequest::invoke(request_id, action, target))
            .and_then(|request| {
                interaction.execute(graph.as_ref(), request, |invocation| {
                    self.apply_invocation(invocation)
                })
            });
        self.interaction = Some(interaction);
        self.finish_interaction(result)
    }

    pub(super) fn finish_interaction(
        &self,
        result: Result<patchbay_model::InteractionReceipt, patchbay_model::InteractionError>,
    ) -> Result<(), String> {
        let receipt = result.map_err(|error| format!("interaction execution: {error:?}"))?;
        match receipt.disposition {
            InteractionDisposition::Succeeded => Ok(()),
            InteractionDisposition::Refused(PatchbayRefusal::IncompatiblePorts) => {
                Err("Ports cannot connect because their Info or temporal contracts differ".into())
            }
            InteractionDisposition::Refused(PatchbayRefusal::DuplicateCord) => {
                Err("those Ports already have a Cord".into())
            }
            InteractionDisposition::Refused(PatchbayRefusal::InvalidConfiguration) => {
                Err("That value does not fit the type or visible bounds on this Gear Face".into())
            }
            InteractionDisposition::Refused(reason) => {
                Err(format!("interaction refused: {reason:?}"))
            }
            InteractionDisposition::Failed => Err("interaction failed".into()),
        }
    }

    pub(super) fn apply_invocation(
        &mut self,
        invocation: &PatchbayInvocation,
    ) -> PatchbayInvocationOutcome {
        if invocation.action == PatchbayAction::PlaceGear {
            return self.apply_palette_placement(&invocation.target_identity);
        }
        if matches!(
            invocation.action,
            PatchbayAction::DuplicateGear
                | PatchbayAction::RemoveGear
                | PatchbayAction::RemoveCord
                | PatchbayAction::ConnectPorts
                | PatchbayAction::RerouteCord
                | PatchbayAction::ConfigureGear
        ) {
            return self.apply_authoring_edit(invocation);
        }
        let current_target = match self
            .graphical_form
            .as_ref()
            .map(|graph| graph.expanded_form_id.as_str().to_owned())
            .or_else(|| {
                self.form_editor
                    .as_ref()
                    .map(|editor| editor.view().open_form)
            }) {
            Some(target) => target,
            None => {
                return PatchbayInvocationOutcome::Refused(PatchbayRefusal::OperationUnavailable)
            }
        };
        if invocation.target_identity != current_target {
            return PatchbayInvocationOutcome::Refused(PatchbayRefusal::StalePresentation);
        }
        let result = match invocation.action {
            PatchbayAction::OpenBack => self.open_next_form(),
            PatchbayAction::Save => match self.form_editor.as_mut() {
                Some(editor) => save_form_resource(editor)
                    .and_then(|()| save_layout_resource(editor, &self.layout)),
                None => Err("canonical Form editor is absent".into()),
            },
            PatchbayAction::ToggleLinearView => {
                self.linear_view = !self.linear_view;
                Ok(())
            }
            PatchbayAction::Birth => self.birth_body(),
            PatchbayAction::Wake => self.wake_body(),
            PatchbayAction::Lull => self.lull_body(),
            PatchbayAction::Plan => self.plan_play(),
            PatchbayAction::Play => self.play_plan(),
            PatchbayAction::Stop => self.control.stop(),
            PatchbayAction::Hold => self.mark_unsatisfied(),
            PatchbayAction::PlaceGear => unreachable!("palette placement returned above"),
            PatchbayAction::DuplicateGear
            | PatchbayAction::RemoveGear
            | PatchbayAction::RemoveCord
            | PatchbayAction::ConnectPorts
            | PatchbayAction::RerouteCord
            | PatchbayAction::ConfigureGear => unreachable!("authoring edit returned above"),
        };
        match result {
            Ok(()) => PatchbayInvocationOutcome::Succeeded,
            Err(_) => PatchbayInvocationOutcome::Failed,
        }
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

    pub(super) fn refresh_graphical_form(&mut self) -> Result<(), String> {
        self.graphical_form = self
            .form_editor
            .as_ref()
            .map(graphical_form_for_editor)
            .transpose()?
            .flatten();
        if let Some(graph) = &self.graphical_form {
            self.layout.reconcile(graph);
        }
        Ok(())
    }

    pub(super) fn selected_graphical_identity(&self) -> Option<&str> {
        let graph = self.graphical_form.as_ref()?;
        let selected = self.interaction.as_ref()?.selected()?;
        graph.resolve_subject_ref(selected).ok()?;
        Some(&selected.subject_identity)
    }

    fn selected_graphical_subject(&self) -> Option<patchbay_model::PatchbaySubjectRef> {
        let graph = self.graphical_form.as_ref()?;
        let selected = self.interaction.as_ref()?.selected()?;
        graph.resolve_subject_ref(selected).ok()?;
        Some(selected.clone())
    }

    fn move_graphical_selection(&mut self, forward: bool) -> Result<(), String> {
        let graph = self
            .graphical_form
            .as_ref()
            .ok_or("graphical Form projection is absent")?;
        let identities = graph.subject_identities().collect::<Vec<_>>();
        let count = identities.len();
        if count == 0 {
            return Ok(());
        }
        let current = self
            .interaction
            .as_ref()
            .and_then(PatchbayInteraction::selected)
            .and_then(|selected| graph.resolve_subject_ref(selected).ok())
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        };
        let subject = graph
            .subject_ref(identities[next])
            .map_err(|error| error.to_string())?;
        self.dispatch_selection(subject)
    }

    pub(super) fn handle_form_key(&mut self, key: &Key) -> Result<bool, String> {
        if self.form_editor.is_none() {
            return Ok(false);
        }
        let mut synchronize_linear_selection = true;
        match key {
            Key::Character(character)
                if self.modifiers.control_key()
                    && character.eq_ignore_ascii_case("d")
                    && !self.linear_view =>
            {
                let subject = self
                    .selected_graphical_subject()
                    .ok_or("select a Gear before duplicating it")?;
                self.handle_gui_action(GuiAction::DuplicateGear(subject))?;
                synchronize_linear_selection = false;
            }
            Key::Character(character)
                if self.modifiers.control_key()
                    && character.eq_ignore_ascii_case("g")
                    && !self.linear_view =>
            {
                let subject = self
                    .selected_graphical_subject()
                    .ok_or("select a Gear before grouping it")?;
                let graph = self
                    .graphical_form
                    .as_ref()
                    .ok_or("graphical Form projection is absent")?;
                self.layout
                    .group_gear(graph, &subject, Some("group-1".into()))
                    .map_err(|error| format!("cannot group Gear: {error:?}"))?;
                synchronize_linear_selection = false;
            }
            Key::Named(NamedKey::Delete) if !self.linear_view => {
                let subject = self
                    .selected_graphical_subject()
                    .ok_or("select a Gear or Cord before removing it")?;
                let action = match self
                    .graphical_form
                    .as_ref()
                    .and_then(|graph| graph.inspect(&subject.subject_identity).ok())
                    .map(|inspection| inspection.subject_kind)
                {
                    Some(patchbay_model::PatchbaySubjectKind::Gear) => {
                        GuiAction::RemoveGear(subject)
                    }
                    Some(patchbay_model::PatchbaySubjectKind::Cord) => {
                        GuiAction::RemoveCord(subject)
                    }
                    _ => return Err("select a Gear or Cord before removing it".into()),
                };
                self.handle_gui_action(action)?;
                synchronize_linear_selection = false;
            }
            Key::Named(NamedKey::Backspace) => self.edit_source(|source| {
                source.pop();
            })?,
            Key::Named(NamedKey::Enter) => self.edit_source(|source| source.push('\n'))?,
            Key::Named(NamedKey::Tab) => self.dispatch_invocation(PatchbayAction::OpenBack)?,
            Key::Named(NamedKey::ArrowDown) | Key::Named(NamedKey::ArrowRight)
                if self.graphical_form.is_some() && !self.linear_view =>
            {
                self.move_graphical_selection(true)?;
                synchronize_linear_selection = false;
            }
            Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowLeft)
                if self.graphical_form.is_some() && !self.linear_view =>
            {
                self.move_graphical_selection(false)?;
                synchronize_linear_selection = false;
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
            Key::Named(NamedKey::F2) => {
                self.dispatch_invocation(PatchbayAction::ToggleLinearView)?
            }
            Key::Named(NamedKey::F4) => self.dispatch_invocation(PatchbayAction::Birth)?,
            Key::Named(NamedKey::F5) => self.dispatch_invocation(PatchbayAction::Wake)?,
            Key::Named(NamedKey::F6) => self.dispatch_invocation(PatchbayAction::Plan)?,
            Key::Named(NamedKey::F7) if !self.modifiers.alt_key() => {
                self.dispatch_invocation(PatchbayAction::Play)?
            }
            Key::Named(NamedKey::F8) if !self.modifiers.alt_key() => {
                self.dispatch_invocation(PatchbayAction::Hold)?
            }
            Key::Named(NamedKey::F9) if !self.modifiers.alt_key() => {
                self.dispatch_invocation(PatchbayAction::Lull)?
            }
            Key::Named(NamedKey::Escape) => self.dispatch_invocation(PatchbayAction::Stop)?,
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
                self.dispatch_invocation(PatchbayAction::Save)?;
            }
            Key::Character(character)
                if !self.modifiers.control_key() && !self.modifiers.super_key() =>
            {
                let characters = character.clone();
                self.edit_source(|source| source.push_str(&characters))?;
            }
            _ => return Ok(false),
        }
        if synchronize_linear_selection {
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
        }
        let title = self.title();
        if let Some(window) = &self.window {
            window.set_title(&title);
            window.request_redraw();
        }
        Ok(true)
    }
}
