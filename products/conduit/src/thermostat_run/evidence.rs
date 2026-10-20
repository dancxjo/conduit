//! Requested post-Play reconstruction; numeric recording and validation stay authoritative.
use crate::plot_source::CanonicalSource;
use conduit_body::BodyPlan;
use conduit_core::{HostAdvertisement, Plan};
use conduit_std_host::body_execution::BodyRunReport;
use serde_json::Value;

pub(super) struct NativeEvidence {
    pub child_signs: Vec<Value>,
    pub report: Value,
}

pub(super) fn render(
    requested: bool,
    source: &CanonicalSource,
    advertisement: &HostAdvertisement,
    source_plan: &Plan,
    body_plan: &BodyPlan,
    report: &BodyRunReport,
) -> Result<Option<NativeEvidence>, String> {
    let children = report
        .scan_child_signs
        .as_ref()
        .map_err(|e| format!("child Signs: {e:?}"))?;
    // A missing mandatory child receipt must refuse even without a descriptive report.
    if !requested {
        return Ok(None);
    }
    let child_signs = children.iter().map(|child| serde_json::json!({
        "parent_active_play_id": child.parent_active_play_id, "activation_id":child.activation_id,
        "selected_plan_id":child.selected_plan_id,"invocation":child.invocation,"child_host_id":child.child_host_id,
        "child_active_play_id":child.child_active_play_id,"events":child.events.iter().map(event_json).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    let native = serde_json::json!({"schema":"conduit.thermostat/native-body-run@1","proof_class":"local-hosted",
        "authored_source":source.source, "advertisement": advertisement, "source_plan": source_plan, "body_plan":body_plan,
        "play":report.play,"terminal":report.terminal,"terminal_sign":report.terminal_sign,
        "kernel_events":report.kernel_events.iter().map(event_json).collect::<Vec<_>>(),"scan_child_signs":child_signs,
        "failure":report.failure,"cleanup_failure":report.cleanup_failure,
        "scan_cancellation_failed":report.scan_cancellation_failed,"scan_output_completion_failed":report.scan_output_completion_failed,
        "outputs":report.fore_deliveries.iter().map(|delivery| serde_json::json!({"front_port_id":delivery.front_port_id,"value_kind":delivery.value_kind,"sequence":delivery.sequence,"track":delivery.track,"bytes":delivery.bytes})).collect::<Vec<_>>()});
    Ok(Some(NativeEvidence {
        child_signs,
        report: native,
    }))
}
fn event_json(event: &conduit_kernel::KernelEvent) -> serde_json::Value {
    serde_json::json!({"sequence":event.sequence,"node":event.node.0,"port":event.port.map(|p|p.0),"request":event.request.map(|r|r.0),"kind":format!("{:?}",event.kind)})
}

#[cfg(test)]
mod tests;
