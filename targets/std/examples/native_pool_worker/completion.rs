//! Project actual adapter-call measurements after execution, before release.
//! These intervals exclude queue wait, warmup, and session transport.
use conduit_std_host::AdmittedLocalModelPoolMember;
use serde_json::{json, Value};

pub(super) fn prepare(member: &AdmittedLocalModelPoolMember) -> Value {
    match member.prepare_completed_work() {
        Ok(receipt) => {
            let placement = receipt.realization();
            json!({
                "phase":"measured",
                "schema":"conduit.proof/native-completed-work@1",
                "scope":"adapter_call_only",
                "selection":receipt.selection(),
                "host_id":placement.host_id,
                "boot_id":placement.boot_id,
                "offer_generation":placement.offer_generation,
                "capability_id":placement.capability_id,
                "implementation_id":placement.implementation_id,
                "artifact_id":placement.artifact_id,
                "started":receipt.started(),
                "completed":receipt.completed(),
                "elapsed":receipt.elapsed(),
                "input_bytes":receipt.input_bytes(),
                "output_bytes":receipt.output_bytes(),
                "input_digest":receipt.input_digest(),
                "terminal":format!("{:?}",receipt.terminal()),
            })
        }
        Err(refusal) => json!({"phase":"measurement-refused","refusal":format!("{refusal:?}")}),
    }
}

/// Two real reads from the owner's identified process clock, outside Play.
pub(super) fn clock(host: &conduit_std_host::StdHost) -> Result<Value, String> {
    use conduit_std_host::{ThreadTimer, TimerAdapter};
    let owner = host.advertisement();
    let mut timer = ThreadTimer;
    let received = timer
        .monotonic_observation(&owner.host_id, &owner.boot_id)
        .ok_or("owner clock receive unavailable")?;
    let sent = timer
        .monotonic_observation(&owner.host_id, &owner.boot_id)
        .ok_or("owner clock send unavailable")?;
    Ok(
        json!({"phase":"clock-observed","host_id":owner.host_id,"boot_id":owner.boot_id,
        "peer_receive":received,"peer_send":sent}),
    )
}
