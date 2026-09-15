//! Bounded resident-application projection of authoritative Presenter truth.

use alloc::{format, string::String, vec::Vec};
use conduit_core::PlanId;
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationViewNode,
};

pub const CHANGE_PRESENTERS_ACTION_ID: &str = "patchbay.presenters.change";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchbayPresenterMode {
    Graphical,
    GraphicalAndSpeech,
    Speech,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchbayPresenterStage {
    pub manifestation_id: String,
    pub implementation_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub resource_pool_id: String,
    pub resource_class_id: String,
    pub reserved_units: u32,
    pub maximum_active_instances: u16,
    pub maximum_queue_items: u16,
    pub maximum_queue_bytes: u32,
    pub available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchbayPresenterTopology {
    pub presentation_id: String,
    pub body_plan_id: PlanId,
    pub active_play_id: String,
    pub mode: PatchbayPresenterMode,
    pub stages: Vec<PatchbayPresenterStage>,
}

impl PatchbayPresenterTopology {
    pub(crate) const fn next_mode(&self) -> PatchbayPresenterMode {
        match self.mode {
            PatchbayPresenterMode::Graphical => PatchbayPresenterMode::GraphicalAndSpeech,
            PatchbayPresenterMode::GraphicalAndSpeech => PatchbayPresenterMode::Speech,
            PatchbayPresenterMode::Speech => PatchbayPresenterMode::GraphicalAndSpeech,
        }
    }
}

pub(crate) fn action() -> ApplicationAction {
    ApplicationAction {
        id: CHANGE_PRESENTERS_ACTION_ID.into(),
        event: ApplicationEventKind::Activate,
    }
}

pub(crate) fn append_nodes(
    topology: &PatchbayPresenterTopology,
    nodes: &mut Vec<ApplicationViewNode>,
    action: u8,
) {
    nodes.push(node(
        ApplicationComponent::Heading,
        "presenter-topology",
        "Presenter topology",
        "",
        0,
        None,
    ));
    definition(
        nodes,
        "presenter-identity",
        "Presentation",
        &topology.presentation_id,
    );
    definition(
        nodes,
        "presenter-body-plan",
        "Body Plan",
        topology.body_plan_id.as_str(),
    );
    definition(nodes, "presenter-play", "Play", &topology.active_play_id);
    for (index, stage) in topology.stages.iter().enumerate() {
        nodes.push(node(
            ApplicationComponent::Status,
            &format!("presenter-stage-{index}"),
            if stage.available {
                "Presenter stage available"
            } else {
                "Presenter stage unavailable"
            },
            "",
            0,
            None,
        ));
        definition(
            nodes,
            &format!("presenter-impl-{index}"),
            "Implementation",
            &stage.implementation_id,
        );
        definition(
            nodes,
            &format!("presenter-facts-{index}"),
            "Manifestation · Host/Boot · reserved resource · finite capacity",
            &format!(
                "{} · {}/{} · {}/{}:{} · instances {} · queue {}/{} bytes",
                stage.manifestation_id,
                stage.host_id,
                stage.boot_id,
                stage.resource_pool_id,
                stage.resource_class_id,
                stage.reserved_units,
                stage.maximum_active_instances,
                stage.maximum_queue_items,
                stage.maximum_queue_bytes,
            ),
        );
    }
    let label = match topology.mode {
        PatchbayPresenterMode::Graphical => "Add speech Presenter",
        PatchbayPresenterMode::GraphicalAndSpeech => "Remove graphical Presenter",
        PatchbayPresenterMode::Speech => "Restore graphical Presenter",
    };
    nodes.push(node(
        ApplicationComponent::Button,
        "change-presenters",
        label,
        "",
        0,
        Some(action),
    ));
}

fn definition(nodes: &mut Vec<ApplicationViewNode>, key: &str, text: &str, value: &str) {
    nodes.push(node(
        ApplicationComponent::Definition,
        key,
        text,
        value,
        768,
        None,
    ));
}

fn node(
    component: ApplicationComponent,
    key: &str,
    text: &str,
    value: &str,
    value_capacity: u32,
    action: Option<u8>,
) -> ApplicationViewNode {
    ApplicationViewNode {
        parent: Some(1),
        component,
        key: key.into(),
        text: text.into(),
        value: value.into(),
        value_capacity,
        action,
        state: ApplicationNodeState::Ready,
    }
}
