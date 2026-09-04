//! Finite browser-memory Line frames and readable exact-Plan projection.

use conduit_core::{bind_active_play, Plan};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LineFrame {
    pub(super) schema: String,
    pub(super) phase: String,
    pub(super) plan_id: String,
    pub(super) source_fragment_id: String,
    pub(super) sink_fragment_id: String,
    pub(super) source_host_id: String,
    pub(super) source_boot_id: String,
    pub(super) source_active_play_id: String,
    pub(super) sink_host_id: String,
    pub(super) sink_boot_id: String,
    pub(super) sink_active_play_id: String,
    pub(super) source_endpoint_id: String,
    pub(super) sink_endpoint_id: String,
    pub(super) connection_id: String,
    pub(super) line_id: String,
    pub(super) link_binding_id: String,
    pub(super) base_implementation_id: String,
    pub(super) base_instance_id: String,
    pub(super) sequence: u64,
    pub(super) value_kind: String,
    pub(super) payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct PlanProjection {
    pub(super) schema: &'static str,
    pub(super) explanation: &'static str,
    pub(super) source_document_id: String,
    pub(super) checked_form_id: String,
    pub(super) expanded_form_id: String,
    pub(super) plan_id: String,
    pub(super) hosts: Vec<HostProjection>,
    pub(super) cord: CordProjection,
    pub(super) raw_plan: Plan,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct HostProjection {
    pub(super) label: &'static str,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) fragment_id: String,
    pub(super) active_play_id: String,
    pub(super) gears: Vec<GearProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct GearProjection {
    pub(super) gear_id: String,
    pub(super) kind_id: String,
    pub(super) implementation_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct CordProjection {
    pub(super) connection_id: String,
    pub(super) value_kind: String,
    pub(super) source_placement_id: String,
    pub(super) sink_placement_id: String,
    pub(super) crosses_host: bool,
    pub(super) line_id: String,
    pub(super) base_implementation_id: String,
    pub(super) base_instance_id: String,
    pub(super) link_binding_id: String,
    pub(super) maximum_in_flight_items: u16,
    pub(super) maximum_payload_bytes: u32,
    pub(super) maximum_buffered_bytes: u32,
}

#[derive(Debug, Serialize)]
#[serde(tag = "effect_kind")]
pub(super) enum Output {
    #[serde(rename = "line")]
    Line {
        schema: &'static str,
        frame: Box<LineFrame>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plan_projection: Option<Box<PlanProjection>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        receipt: Option<Box<MultiHostReceipt>>,
    },
    #[serde(rename = "waiting")]
    Waiting {
        schema: &'static str,
        phase: &'static str,
        plan_id: String,
    },
    #[serde(rename = "manifestation")]
    Manifestation {
        schema: &'static str,
        manifestation: Box<super::super::protocol::TourEffect>,
        accepted_frame: Box<LineFrame>,
        plan_projection: Box<PlanProjection>,
    },
    #[serde(rename = "receipt")]
    Receipt {
        schema: &'static str,
        receipt: Box<MultiHostReceipt>,
    },
}

#[derive(Debug, Serialize)]
pub(super) struct MultiHostReceipt {
    pub(super) schema: &'static str,
    pub(super) disposition: &'static str,
    pub(super) plan_id: String,
    pub(super) fragment_id: String,
    pub(super) active_play_id: String,
    pub(super) host_id: String,
    pub(super) boot_id: String,
    pub(super) terminal_sign_id: String,
    pub(super) transferred_values: u32,
}

pub(super) fn projection(plan: &Plan, play_sequence: u64) -> Result<PlanProjection, String> {
    if plan.fragments.len() != 2 {
        return Err("multi-Host Plan projection requires exactly two fragments".into());
    }
    let connection = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find(|connection| connection.selected_line.is_some())
        .ok_or_else(|| "multi-Host Plan projection has no selected Line".to_string())?;
    let line = connection
        .selected_line
        .as_ref()
        .ok_or_else(|| "multi-Host Cord has no exact selected Line".to_string())?;
    let mut hosts = Vec::with_capacity(2);
    for fragment in &plan.fragments {
        let play = bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        hosts.push(HostProjection {
            label: if fragment.host_id == line.binding.source.host_id {
                "Host A"
            } else {
                "Host B"
            },
            host_id: fragment.host_id.as_str().into(),
            boot_id: fragment.boot_id.as_str().into(),
            fragment_id: fragment.fragment_id.as_str().into(),
            active_play_id: play.active_play_id.as_str().into(),
            gears: fragment
                .placements
                .iter()
                .map(|placement| GearProjection {
                    gear_id: placement.gear_id.as_str().into(),
                    kind_id: placement.kind_id.as_str().into(),
                    implementation_id: placement.implementation_id.as_str().into(),
                })
                .collect(),
        });
    }
    hosts.sort_by(|left, right| left.label.cmp(right.label));
    Ok(PlanProjection {
        schema: "conduit.tour/plan-projection@1",
        explanation: "The Form says what; current Host offers constrain what is possible; this immutable Plan says exactly how this Play is realized.",
        source_document_id: plan.source_document_id.as_str().into(),
        checked_form_id: plan.checked_form_id.as_str().into(),
        expanded_form_id: plan.expanded_form_id.as_str().into(),
        plan_id: plan.plan_id.as_str().into(),
        hosts,
        cord: CordProjection {
            connection_id: connection.connection_id.as_str().into(),
            value_kind: connection.value_kind.as_str().into(),
            source_placement_id: connection.source_placement_id.as_str().into(),
            sink_placement_id: connection.sink_placement_id.as_str().into(),
            crosses_host: true,
            line_id: line.line_id.as_str().into(),
            base_implementation_id: line.binding.base.as_str().into(),
            base_instance_id: line.binding.base_instance_id.as_str().into(),
            link_binding_id: line.binding.binding_id.as_str().into(),
            maximum_in_flight_items: line.binding.limits.maximum_in_flight_items,
            maximum_payload_bytes: line.binding.limits.maximum_payload_bytes,
            maximum_buffered_bytes: line.binding.limits.maximum_buffered_bytes,
        },
        raw_plan: plan.clone(),
    })
}
