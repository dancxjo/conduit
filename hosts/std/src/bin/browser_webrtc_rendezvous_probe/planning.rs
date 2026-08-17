//! Exact immutable bindings and evidence basis for each admitted grant generation.

use conduit_body::{BodyId, MembershipCredential};
use conduit_core::{
    bind_active_play, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId, KindId,
    LineId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId, PROTOCOL_VERSION,
};
use conduit_wire::{LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits};

pub(super) fn binding(
    source: &MembershipCredential,
    sink: &MembershipCredential,
    generation: u16,
) -> SessionBinding {
    let suffix = if generation == 0 {
        ""
    } else {
        "/replacement-1"
    };
    let plan_id = PlanId::from(format!("plan/browser-webrtc-rendezvous-probe{suffix}"));
    SessionBinding {
        protocol_version: PROTOCOL_VERSION,
        plan_id: plan_id.clone(),
        source_fragment_id: FragmentId::from("fragment/source"),
        sink_fragment_id: FragmentId::from("fragment/sink"),
        source_active_play_id: bind_active_play(&plan_id, &source.host_id, &source.boot_id, 0)
            .active_play_id,
        sink_active_play_id: bind_active_play(&plan_id, &sink.host_id, &sink.boot_id, 0)
            .active_play_id,
        connection_id: ConnectionId::from("connection/browser-webrtc-rendezvous-probe"),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: KindId::from("value/bounded@1"),
        limits: SessionLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: 16,
            maximum_buffered_bytes: 16,
        },
        attachment: LineAttachment {
            line_id: LineId::from(format!("line/browser-webrtc-rendezvous-probe{suffix}")),
            link_binding_id: LinkBindingId::from(format!(
                "binding/browser-webrtc-rendezvous-probe{suffix}"
            )),
            base: ConnectionBase::WebRtcDataChannel,
            base_instance_id: ConnectionBaseInstanceId::from(format!(
                "base/browser-webrtc-rendezvous-probe{suffix}"
            )),
            source_host_id: source.host_id.clone(),
            source_boot_id: source.boot_id.clone(),
            source_endpoint_id: LinkEndpointId::from("endpoint/source"),
            sink_host_id: sink.host_id.clone(),
            sink_boot_id: sink.boot_id.clone(),
            sink_endpoint_id: LinkEndpointId::from("endpoint/sink"),
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 16,
                maximum_buffered_bytes: 16,
                maximum_frame_bytes: 1_024,
            },
        },
    }
}

pub(super) fn session_basis(
    body_id: &BodyId,
    source: &MembershipCredential,
    sink: &MembershipCredential,
    binding: &SessionBinding,
    generation: u16,
) -> serde_json::Value {
    serde_json::json!({
        "generation": generation,
        "body_id": body_id.as_str(),
        "source_part_id": source.part_id.as_str(),
        "sink_part_id": sink.part_id.as_str(),
        "plan_id": binding.plan_id.as_str(),
        "source_active_play_id": binding.source_active_play_id.as_str(),
        "sink_active_play_id": binding.sink_active_play_id.as_str(),
        "connection_id": binding.connection_id.as_str(),
        "line_id": binding.attachment.line_id.as_str(),
        "binding_id": binding.attachment.link_binding_id.as_str(),
        "base": binding.attachment.base,
        "base_instance_id": binding.attachment.base_instance_id.as_str(),
        "value_kind": binding.value_kind.as_str(),
        "session_limits": {
            "maximum_in_flight_items": binding.limits.maximum_in_flight_items,
            "maximum_payload_bytes": binding.limits.maximum_payload_bytes,
            "maximum_buffered_bytes": binding.limits.maximum_buffered_bytes,
        },
        "line_limits": binding.attachment.limits,
    })
}
