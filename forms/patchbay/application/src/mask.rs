//! Bounded resident-application projection of authoritative Mask truth.

use alloc::{format, string::String, vec::Vec};
use conduit_core::PlanId;
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationViewNode,
};

pub const CHANGE_MASKS_ACTION_ID: &str = "patchbay.masks.change";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchbayMaskMode {
    Graphical,
    GraphicalAndSpeech,
    Speech,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchbayMaskStage {
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
pub struct PatchbayMaskTopology {
    pub presentation_id: String,
    pub body_plan_id: PlanId,
    pub active_play_id: String,
    pub mode: PatchbayMaskMode,
    pub stages: Vec<PatchbayMaskStage>,
}

impl PatchbayMaskTopology {
    pub(crate) const fn next_mode(&self) -> PatchbayMaskMode {
        match self.mode {
            PatchbayMaskMode::Graphical => PatchbayMaskMode::GraphicalAndSpeech,
            PatchbayMaskMode::GraphicalAndSpeech => PatchbayMaskMode::Speech,
            PatchbayMaskMode::Speech => PatchbayMaskMode::GraphicalAndSpeech,
        }
    }
}

pub(crate) fn action() -> ApplicationAction {
    ApplicationAction {
        id: CHANGE_MASKS_ACTION_ID.into(),
        event: ApplicationEventKind::Activate,
    }
}

pub(crate) fn append_nodes(
    topology: &PatchbayMaskTopology,
    nodes: &mut Vec<ApplicationViewNode>,
    action: u8,
) {
    nodes.push(node(
        ApplicationComponent::Heading,
        "mask-topology",
        "Mask topology",
        "",
        0,
        None,
    ));
    definition(nodes, "face-identity", "Face", &topology.presentation_id);
    definition(
        nodes,
        "mask-body-plan",
        "Body Plan",
        topology.body_plan_id.as_str(),
    );
    definition(nodes, "mask-play", "Play", &topology.active_play_id);
    for (index, stage) in topology.stages.iter().enumerate() {
        nodes.push(node(
            ApplicationComponent::Status,
            &format!("mask-stage-{index}"),
            if stage.available {
                "Mask stage available"
            } else {
                "Mask stage unavailable"
            },
            "",
            0,
            None,
        ));
        definition(
            nodes,
            &format!("mask-impl-{index}"),
            "Implementation",
            &stage.implementation_id,
        );
        definition(
            nodes,
            &format!("mask-facts-{index}"),
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
        PatchbayMaskMode::Graphical => "Wear speech Mask",
        PatchbayMaskMode::GraphicalAndSpeech => "Doff graphical Mask",
        PatchbayMaskMode::Speech => "Wear graphical Mask",
    };
    nodes.push(node(
        ApplicationComponent::Button,
        "change-masks",
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
