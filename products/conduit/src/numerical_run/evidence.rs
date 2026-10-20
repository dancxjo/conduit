//! Post-Play serialization borrows the actual Body and child evidence.
use crate::plot_source::CanonicalSource;
use conduit_body::BodyPlan;
use conduit_core::{HostAdvertisement, Plan};
use conduit_std_host::body_execution::BodyRunReport;

pub(super) fn render(
    source: &CanonicalSource,
    host: &HostAdvertisement,
    plan: &Plan,
    body: &BodyPlan,
    report: &BodyRunReport,
) -> Result<serde_json::Value, String> {
    let children = report
        .each_child_signs
        .as_ref()
        .map_err(|e| format!("numerical child evidence: {e:?}"))?;
    let children=children.iter().map(|child|serde_json::json!({
        "parent_active_play_id":child.parent_active_play_id,"activation_id":child.activation_id,
        "selected_plan_id":child.selected_plan_id,"invocation":child.invocation,"child_host_id":child.child_host_id,
        "child_active_play_id":child.child_active_play_id,"events":child.events.iter().map(event).collect::<Vec<_>>(),
        "remote_lifecycle":child.remote_lifecycle.iter().map(|(sequence,identity)|serde_json::json!({
            "event_sequence":sequence,"endpoint":identity.endpoint.0,"cord":identity.cord.0,"direction":format!("{:?}",identity.direction),"sequence":identity.sequence
        })).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    Ok(
        serde_json::json!({"schema":"conduit.numerical-fore/native-body-run@1","proof_class":"local-hosted",
            "authored_source":source.source,"advertisement":host,"source_plan":plan,"body_plan":body,
            "play":report.play,"terminal":report.terminal,"terminal_sign":report.terminal_sign,
            "kernel_events":report.kernel_events.iter().map(event).collect::<Vec<_>>(),"each_child_signs":children,
            "failure":report.failure,"kernel_failure":report.kernel_failure.map(|failure|format!("{failure:?}")),"cleanup_failure":report.cleanup_failure,
            "each_cancellation_failed":report.each_cancellation_failed,"each_output_completion_failed":report.each_output_completion_failed,
            "outputs":report.fore_deliveries.iter().map(|delivery|serde_json::json!({"front_port_id":delivery.front_port_id,"value_kind":delivery.value_kind,"sequence":delivery.sequence,"track":delivery.track,"bytes":delivery.bytes})).collect::<Vec<_>>()
        }),
    )
}
fn event(event: &conduit_kernel::KernelEvent) -> serde_json::Value {
    serde_json::json!({"sequence":event.sequence,"node":event.node.0,"port":event.port.map(|port|port.0),"request":event.request.map(|request|request.0),"kind":format!("{:?}",event.kind)})
}
