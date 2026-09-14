//! Retained, renderer-neutral Patchbay application over exact Form/Plan truth.
#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::PlanId;
use conduit_form::ExpandedCanonicalForm;
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEvent, ApplicationEventKind,
    ApplicationNodeState, ApplicationView, ApplicationViewNode, ApplicationViewRefusal,
};
use patchbay_control::PatchbayAction;
use patchbay_graph::{PatchbayGraph, PatchbayGraphError, PatchbayInspection};

pub const INSPECT_NEXT_ACTION_ID: &str = "patchbay.inspect.next";
pub const EDIT_CURRENT_ACTION_ID: &str = "patchbay.edit.current";
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
}

pub struct PatchbayApplicationPort {
    graph: PatchbayGraph,
    plan_id: PlanId,
    body_plan_id: PlanId,
    revision: u32,
    selected: usize,
    edit_requested: bool,
    presenter_topology: Option<PatchbayPresenterTopology>,
}

impl PatchbayApplicationPort {
    pub fn open(
        form: &ExpandedCanonicalForm,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        Ok(Self {
            graph: PatchbayGraph::from_expanded(form).map_err(PatchbayApplicationRefusal::Graph)?,
            plan_id,
            body_plan_id,
            revision: 1,
            selected: 0,
            edit_requested: false,
            presenter_topology: None,
        })
    }

    pub fn set_presenter_topology(&mut self, topology: PatchbayPresenterTopology) {
        self.body_plan_id = topology.body_plan_id.clone();
        self.presenter_topology = Some(topology);
    }

    pub const fn graph(&self) -> &PatchbayGraph {
        &self.graph
    }
    pub const fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn inspection(&self) -> Result<PatchbayInspection, PatchbayApplicationRefusal> {
        let subject = self
            .graph
            .subject_identities()
            .nth(self.selected)
            .ok_or(PatchbayApplicationRefusal::NoSubject)?;
        self.graph
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
                let count = self.graph.subject_identities().count();
                if count == 0 {
                    return Err(PatchbayApplicationRefusal::NoSubject);
                }
                self.selected = (self.selected + 1) % count;
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
                    expanded_form_id: self.graph.expanded_form_id.clone(),
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
            _ => Err(PatchbayApplicationRefusal::UnknownAction),
        }
    }

    fn view(&self) -> Result<ApplicationView, PatchbayApplicationRefusal> {
        let inspection = self.inspection()?;
        let mut actions = vec![
            ApplicationAction {
                id: INSPECT_NEXT_ACTION_ID.into(),
                event: ApplicationEventKind::Activate,
            },
            ApplicationAction {
                id: EDIT_CURRENT_ACTION_ID.into(),
                event: ApplicationEventKind::Activate,
            },
        ];
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
                "current",
                "Current flow",
                "",
                0,
                None,
            ),
            node(
                Some(1),
                ApplicationComponent::Heading,
                "form",
                &self.graph.form_name,
                "",
                0,
                None,
            ),
            node(
                Some(1),
                ApplicationComponent::Status,
                "identity",
                &format!(
                    "Form {} · Plan {} · Body Plan {}",
                    self.graph.expanded_form_id.as_str(),
                    self.plan_id.as_str(),
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
                Some(0),
            ),
            node(
                Some(1),
                ApplicationComponent::Button,
                "edit-current",
                "Edit current",
                "",
                0,
                Some(if self.presenter_topology.is_some() {
                    2
                } else {
                    1
                }),
            ),
        ];
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
            presenter::append_nodes(topology, &mut nodes);
        }
        let view = ApplicationView {
            revision: self.revision,
            nodes,
            actions,
        };
        view.validate().map_err(PatchbayApplicationRefusal::Event)?;
        Ok(view)
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
mod tests {
    use super::*;
    use conduit_form::{ProfileCatalog, StartupCatalog};

    fn form() -> ExpandedCanonicalForm {
        let mut startup = StartupCatalog::new();
        let mut profile = ProfileCatalog::new();
        conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile).unwrap();
        let syntax = conduit_form::parse_syntax_document(include_str!(
            "../../../../forms/tour/main.conduit"
        ));
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        conduit_form::expand_canonical_form(&checked, "tour", &profile).unwrap()
    }

    #[test]
    fn opens_the_exact_form_and_plan_and_retains_inspection() {
        let form = form();
        let mut port = PatchbayApplicationPort::open(
            &form,
            PlanId::from("plan/current"),
            PlanId::from("body-plan/current"),
        )
        .unwrap();
        let initial = port.apply(&[]).unwrap();
        let view = ApplicationView::decode(&initial.view).unwrap();
        let event = ApplicationEvent {
            revision: view.revision,
            action: INSPECT_NEXT_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        };
        let next = port.apply(&event.encode(&view).unwrap()).unwrap();
        assert_eq!(ApplicationView::decode(&next.view).unwrap().revision, 2);
        assert_eq!(port.graph().expanded_form_id, form.expanded_form_id);
        assert_eq!(port.plan_id().as_str(), "plan/current");
    }

    #[test]
    fn stale_events_and_edit_authority_remain_explicit() {
        let form = form();
        let mut port = PatchbayApplicationPort::open(
            &form,
            PlanId::from("plan/current"),
            PlanId::from("body-plan/current"),
        )
        .unwrap();
        let view = ApplicationView::decode(&port.apply(&[]).unwrap().view).unwrap();
        let edit = ApplicationEvent {
            revision: view.revision,
            action: EDIT_CURRENT_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        };
        assert!(matches!(
            port.apply(&edit.encode(&view).unwrap()).unwrap().request,
            Some(PatchbayApplicationRequest::EditCurrent { .. })
        ));
        let stale = ApplicationEvent {
            revision: view.revision,
            action: INSPECT_NEXT_ACTION_ID.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        };
        let stale_view = ApplicationView {
            revision: stale.revision,
            ..view
        };
        assert_eq!(
            port.apply(&stale.encode(&stale_view).unwrap()),
            Err(PatchbayApplicationRefusal::Event(
                ApplicationViewRefusal::StaleRevision
            ))
        );
    }

    #[test]
    fn presenter_topology_is_visible_and_requests_the_exact_body_plan() {
        let form = form();
        let mut port = PatchbayApplicationPort::open(
            &form,
            PlanId::from("plan/current"),
            PlanId::from("body-plan/current"),
        )
        .unwrap();
        port.set_presenter_topology(PatchbayPresenterTopology {
            presentation_id: "presentation/current".into(),
            body_plan_id: PlanId::from("body-plan/current"),
            active_play_id: "play/current".into(),
            mode: PatchbayPresenterMode::Graphical,
            stages: vec![PatchbayPresenterStage {
                manifestation_id: "manifestation/graphical".into(),
                implementation_id: "presentation/renderer-conduitos-native@1".into(),
                host_id: "host/current".into(),
                boot_id: "boot/current".into(),
                resource_pool_id: "pool/presenter".into(),
                resource_class_id: "resource/presenter".into(),
                reserved_units: 1,
                maximum_active_instances: 1,
                maximum_queue_items: 1,
                maximum_queue_bytes: 4096,
                available: true,
            }],
        });
        let output = port.apply(&[]).unwrap();
        let view = ApplicationView::decode(&output.view).unwrap();
        assert_eq!(view.actions[0].id, INSPECT_NEXT_ACTION_ID);
        assert_eq!(view.actions[1].id, EDIT_CURRENT_ACTION_ID);
        assert_eq!(view.actions[2].id, CHANGE_PRESENTERS_ACTION_ID);
        assert!(view
            .nodes
            .iter()
            .any(|node| { node.key == "presenter-stage-0" && node.text.contains("available") }));
        let action = view
            .actions
            .iter()
            .find(|action| action.id == CHANGE_PRESENTERS_ACTION_ID)
            .unwrap();
        let changed = port
            .apply(
                &ApplicationEvent {
                    revision: view.revision,
                    action: action.id.clone(),
                    kind: action.event,
                    value: Vec::new(),
                }
                .encode(&view)
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            changed.request,
            Some(PatchbayApplicationRequest::ChangePresenters {
                body_plan_id: PlanId::from("body-plan/current"),
                mode: PatchbayPresenterMode::GraphicalAndSpeech,
            })
        );
    }
}
