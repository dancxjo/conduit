//! Retained, renderer-neutral Patchbay application over exact Form/Plan truth.
#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{ActivePlayId, PlanId};
use conduit_form::ExpandedCanonicalForm;
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEvent, ApplicationEventKind,
    ApplicationNodeState, ApplicationView, ApplicationViewNode, ApplicationViewRefusal,
};
use patchbay_control::PatchbayAction;
use patchbay_graph::{PatchbayGraph, PatchbayGraphError, PatchbayInspection};

pub const INSPECT_NEXT_ACTION_ID: &str = "patchbay.inspect.next";
pub const EDIT_CURRENT_ACTION_ID: &str = "patchbay.edit.current";
pub const SELECT_FORM_ACTION_PREFIX: &str = "patchbay.form.";
mod presenter;
pub use presenter::{
    PatchbayPresenterMode, PatchbayPresenterStage, PatchbayPresenterTopology,
    CHANGE_PRESENTERS_ACTION_ID,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchbayApplicationRequest {
    EditCurrent {
        expanded_form_id: conduit_core::ExpandedFormId,
        subject_identity: String,
    },
    ChangePresenters {
        body_plan_id: PlanId,
        mode: PatchbayPresenterMode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchbayApplicationOutput {
    pub view: Vec<u8>,
    /// Authority-bearing changes are requests against authoritative Body truth.
    pub request: Option<PatchbayApplicationRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchbayApplicationRefusal {
    Graph(PatchbayGraphError),
    Event(ApplicationViewRefusal),
    UnknownAction,
    NoSubject,
    RevisionExhausted,
    EmptyActiveForms,
    TooManyActiveForms,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchbayActiveFormState {
    Playing,
    Lulled,
}

pub struct PatchbayActiveForm {
    graph: PatchbayGraph,
    title: String,
    checked_form_id: String,
    plan_id: PlanId,
    state: PatchbayActiveFormState,
    focused: bool,
}

impl PatchbayActiveForm {
    pub fn project(
        form: &ExpandedCanonicalForm,
        title: impl Into<String>,
        checked_form_id: impl Into<String>,
        plan_id: PlanId,
        state: PatchbayActiveFormState,
        focused: bool,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        Ok(Self {
            graph: PatchbayGraph::from_expanded(form).map_err(PatchbayApplicationRefusal::Graph)?,
            title: title.into(),
            checked_form_id: checked_form_id.into(),
            plan_id,
            state,
            focused,
        })
    }
}

pub struct PatchbayApplicationPort {
    forms: Vec<PatchbayActiveForm>,
    body_plan_id: PlanId,
    active_play_id: Option<ActivePlayId>,
    revision: u32,
    selected_form: Option<usize>,
    selected_subject: usize,
    edit_requested: bool,
    presenter_topology: Option<PatchbayPresenterTopology>,
}

impl PatchbayApplicationPort {
    pub fn open(
        form: &ExpandedCanonicalForm,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        let checked_form_id = String::from(form.checked_form_id.as_str());
        let entry = PatchbayActiveForm::project(
            form,
            form.name.clone(),
            checked_form_id,
            plan_id,
            PatchbayActiveFormState::Playing,
            true,
        )?;
        let mut port = Self::open_active(vec![entry], body_plan_id, None)?;
        port.selected_form = Some(0);
        Ok(port)
    }

    pub fn open_active(
        forms: Vec<PatchbayActiveForm>,
        body_plan_id: PlanId,
        active_play_id: Option<ActivePlayId>,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        if forms.is_empty() {
            return Err(PatchbayApplicationRefusal::EmptyActiveForms);
        }
        if forms.len() + 3 > conduit_presentation::MAX_APPLICATION_ACTIONS {
            return Err(PatchbayApplicationRefusal::TooManyActiveForms);
        }
        Ok(Self {
            forms,
            body_plan_id,
            active_play_id,
            revision: 1,
            selected_form: None,
            selected_subject: 0,
            edit_requested: false,
            presenter_topology: None,
        })
    }

    pub fn set_presenter_topology(&mut self, topology: PatchbayPresenterTopology) {
        self.body_plan_id = topology.body_plan_id.clone();
        self.presenter_topology = Some(topology);
    }

    pub fn graph(&self) -> &PatchbayGraph {
        &self.forms[0].graph
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.forms[0].plan_id
    }

    pub fn inspection(&self) -> Result<PatchbayInspection, PatchbayApplicationRefusal> {
        let form = self.selected_active_form()?;
        let subject = form
            .graph
            .subject_identities()
            .nth(self.selected_subject)
            .ok_or(PatchbayApplicationRefusal::NoSubject)?;
        form.graph
            .inspect(subject)
            .map_err(PatchbayApplicationRefusal::Graph)
    }

    pub fn apply(
        &mut self,
        encoded: &[u8],
    ) -> Result<PatchbayApplicationOutput, PatchbayApplicationRefusal> {
        let current = self.view()?;
        let request = if encoded.is_empty() {
            None
        } else {
            let event = ApplicationEvent::decode(encoded, &current)
                .map_err(PatchbayApplicationRefusal::Event)?;
            self.apply_event(&event)?
        };
        Ok(PatchbayApplicationOutput {
            view: self
                .view()?
                .encode()
                .map_err(PatchbayApplicationRefusal::Event)?,
            request,
        })
    }

    fn apply_event(
        &mut self,
        event: &ApplicationEvent,
    ) -> Result<Option<PatchbayApplicationRequest>, PatchbayApplicationRefusal> {
        if event.kind != ApplicationEventKind::Activate {
            return Err(PatchbayApplicationRefusal::UnknownAction);
        }
        match event.action.as_str() {
            INSPECT_NEXT_ACTION_ID => {
                let count = self
                    .selected_active_form()?
                    .graph
                    .subject_identities()
                    .count();
                if count == 0 {
                    return Err(PatchbayApplicationRefusal::NoSubject);
                }
                self.selected_subject = (self.selected_subject + 1) % count;
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                Ok(None)
            }
            EDIT_CURRENT_ACTION_ID => {
                let inspection = self.inspection()?;
                // Reuse the shared semantic action identity; realization remains
                // outside application state and cannot acquire ambient authority.
                debug_assert_eq!(PatchbayAction::ConfigureGear.as_str(), "configure-gear");
                self.edit_requested = true;
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                Ok(Some(PatchbayApplicationRequest::EditCurrent {
                    expanded_form_id: self.selected_active_form()?.graph.expanded_form_id.clone(),
                    subject_identity: inspection.subject_identity,
                }))
            }
            CHANGE_PRESENTERS_ACTION_ID => {
                let topology = self
                    .presenter_topology
                    .as_ref()
                    .ok_or(PatchbayApplicationRefusal::UnknownAction)?;
                let mode = topology.next_mode();
                Ok(Some(PatchbayApplicationRequest::ChangePresenters {
                    body_plan_id: topology.body_plan_id.clone(),
                    mode,
                }))
            }
            action if action.starts_with(SELECT_FORM_ACTION_PREFIX) => {
                let index = action[SELECT_FORM_ACTION_PREFIX.len()..]
                    .parse::<usize>()
                    .map_err(|_| PatchbayApplicationRefusal::UnknownAction)?;
                if index >= self.forms.len() {
                    return Err(PatchbayApplicationRefusal::UnknownAction);
                }
                self.selected_form = Some(index);
                self.selected_subject = 0;
                self.edit_requested = false;
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                Ok(None)
            }
            _ => Err(PatchbayApplicationRefusal::UnknownAction),
        }
    }

    fn view(&self) -> Result<ApplicationView, PatchbayApplicationRefusal> {
        let mut actions = Vec::with_capacity(self.forms.len() + 3);
        for index in 0..self.forms.len() {
            actions.push(ApplicationAction {
                id: format!("{SELECT_FORM_ACTION_PREFIX}{index}"),
                event: ApplicationEventKind::Activate,
            });
        }
        let inspect_action = actions.len();
        actions.push(ApplicationAction {
            id: INSPECT_NEXT_ACTION_ID.into(),
            event: ApplicationEventKind::Activate,
        });
        let edit_action = actions.len();
        actions.push(ApplicationAction {
            id: EDIT_CURRENT_ACTION_ID.into(),
            event: ApplicationEventKind::Activate,
        });
        if self.presenter_topology.is_some() {
            actions.push(presenter::action());
        }
        let mut nodes = vec![
            node(
                None,
                ApplicationComponent::Shell,
                "patchbay",
                "Patchbay",
                "",
                0,
                None,
            ),
            node(
                Some(0),
                ApplicationComponent::Main,
                "active-forms",
                "Active Forms on this Body",
                "",
                0,
                None,
            ),
        ];
        if let Some(active_play_id) = &self.active_play_id {
            nodes.push(node(
                Some(1),
                ApplicationComponent::Status,
                "body-play",
                &format!(
                    "Body Plan {} · Play {}",
                    self.body_plan_id.as_str(),
                    active_play_id.as_str()
                ),
                "",
                0,
                None,
            ));
        }
        for (index, form) in self.forms.iter().enumerate() {
            let state = match form.state {
                PatchbayActiveFormState::Playing => "Playing",
                PatchbayActiveFormState::Lulled => "Lulled",
            };
            let focus = if form.focused { " · foreground" } else { "" };
            nodes.push(node(
                Some(1),
                ApplicationComponent::Button,
                &format!("active-form-{index}"),
                &form.title,
                "",
                0,
                Some(index as u8),
            ));
            nodes.push(node(
                Some(1),
                ApplicationComponent::Status,
                &format!("active-form-status-{index}"),
                &format!("{state}{focus} · checked {}", form.checked_form_id),
                "",
                0,
                None,
            ));
        }
        if let Some(index) = self.selected_form {
            let form = &self.forms[index];
            let inspection = self.inspection()?;
            nodes.extend([
                node(
                    Some(1),
                    ApplicationComponent::Heading,
                    "selected-form",
                    &form.title,
                    "",
                    0,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Status,
                    "identity",
                    &format!(
                        "Expanded Form {} · Plan {} · Body Plan {}",
                        form.graph.expanded_form_id.as_str(),
                        form.plan_id.as_str(),
                        self.body_plan_id.as_str()
                    ),
                    "",
                    0,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Definition,
                    "subject",
                    &format!("{:?}", inspection.subject_kind),
                    &inspection.subject_identity,
                    768,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Paragraph,
                    "facts",
                    &inspection.exact_facts.join(" · "),
                    "",
                    0,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Button,
                    "inspect-next",
                    "Inspect next",
                    "",
                    0,
                    Some(inspect_action as u8),
                ),
                node(
                    Some(1),
                    ApplicationComponent::Button,
                    "edit-current",
                    "Edit current",
                    "",
                    0,
                    Some(edit_action as u8),
                ),
            ]);
        }
        if self.edit_requested {
            nodes.push(node(
                Some(1),
                ApplicationComponent::Status,
                "edit-request",
                "Edit requested against the exact inspected subject; no authority was inferred.",
                "",
                0,
                None,
            ));
        }
        if let Some(topology) = &self.presenter_topology {
            presenter::append_nodes(topology, &mut nodes, (actions.len() - 1) as u8);
        }
        let view = ApplicationView {
            revision: self.revision,
            nodes,
            actions,
        };
        view.validate().map_err(PatchbayApplicationRefusal::Event)?;
        Ok(view)
    }

    fn selected_active_form(&self) -> Result<&PatchbayActiveForm, PatchbayApplicationRefusal> {
        self.selected_form
            .and_then(|index| self.forms.get(index))
            .ok_or(PatchbayApplicationRefusal::NoSubject)
    }
}

fn node(
    parent: Option<u8>,
    component: ApplicationComponent,
    key: &str,
    text: &str,
    value: &str,
    value_capacity: u32,
    action: Option<u8>,
) -> ApplicationViewNode {
    ApplicationViewNode {
        parent,
        component,
        key: key.into(),
        text: text.into(),
        value: value.into(),
        value_capacity,
        action,
        state: ApplicationNodeState::Ready,
    }
}

#[cfg(test)]
mod tests;
