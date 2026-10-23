//! Strict separate stopped-owner lifecycle observations, never product checkpoints.
use super::super::{
    ConduitosError,
    compositor_lifecycle_proof::{self as wire, AckRef, Cost, Identity, Nullable},
};
use serde::Deserialize;
use serde_json::{Value, json};
#[path = "zero_receipt.rs"]
mod zero_receipt;
const PREFIX: &str = "CONDUIT_COMPOSITOR_STOPPED_LIFECYCLE ";
const RESULT: &str = "CONDUIT_COMPOSITOR_STOPPED_LIFECYCLE_RESULT ";
const PHASES: [&str; 8] = [
    "cold-installed",
    "cold-prepared",
    "initial-ack",
    "cancel-before",
    "cancel-service-disposed",
    "cancel-owner-disposed",
    "restart-prepared",
    "restart-ack",
];
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    part: u8,
    phase: String,
    identity: Identity,
    instance: u64,
    namespace: Nullable<u64>,
    ack: Nullable<AckRef>,
    cost: Cost,
    raw_ticks: Nullable<u64>,
    staging_live_bytes: usize,
    staging_prepared_bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultRecord {
    schema: String,
    status: String,
    stopped_owner_cancellation: bool,
    unfinished_upload_fenced: bool,
    staging_released: bool,
    old_clients_refused: bool,
    stale_drop_preserves_replacement: bool,
    same_retained_grant: bool,
    fresh_semantic_show_and_progress: bool,
    whole_zero_audited: bool,
    asynchronous_cancel: bool,
    guest_cpu_fault: bool,
    malformed_reply: bool,
    physical_scanout: bool,
}
fn refusal() -> ConduitosError {
    ConduitosError::refusal(
        "compositor-stopped-lifecycle-mismatch",
        "requires eight exact source/image/grant-bound stopped-owner cancellation/disposal/restart observations and real ACK joins; a full initialized-data zero witness; no async, CPU-fault, malformed-reply or physical claim",
    )
}
pub(super) fn validate(
    serial: &str,
    profile: &str,
    build: &str,
    image: &str,
    guest: [u8; 32],
) -> Result<Value, ConduitosError> {
    let mut records = Vec::with_capacity(8);
    let mut values = Vec::with_capacity(8);
    let mut result = None;
    let mut zero = None;
    for line in serial.split_inclusive('\n') {
        if line.starts_with(zero_receipt::PREFIX.trim_end()) {
            let payload = line
                .strip_prefix(zero_receipt::PREFIX)
                .ok_or_else(refusal)?;
            if !line.ends_with('\n')
                || line.len() > 4096
                || records.len() != 8
                || zero.is_some()
                || result.is_some()
            {
                return Err(refusal());
            }
            zero = Some(zero_receipt::validate(payload, &records)?);
            continue;
        }
        let payload = if let Some(payload) = line.strip_prefix(PREFIX) {
            Some((false, payload))
        } else {
            line.strip_prefix(RESULT).map(|payload| (true, payload))
        };
        let Some((summary, payload)) = payload else {
            continue;
        };
        if !line.ends_with('\n') || line.len() > 4096 {
            return Err(refusal());
        }
        if summary {
            let decoded: ResultRecord = serde_json::from_str(payload).map_err(|_| refusal())?;
            if result.is_some()
                || records.len() != 8
                || zero.is_none()
                || decoded.schema != "conduit.conduitos/compositor-stopped-lifecycle-result@2"
                || decoded.status != "completed"
                || !decoded.stopped_owner_cancellation
                || !decoded.unfinished_upload_fenced
                || !decoded.staging_released
                || !decoded.old_clients_refused
                || !decoded.stale_drop_preserves_replacement
                || !decoded.same_retained_grant
                || !decoded.fresh_semantic_show_and_progress
                || !decoded.whole_zero_audited
                || decoded.asynchronous_cancel
                || decoded.guest_cpu_fault
                || decoded.malformed_reply
                || decoded.physical_scanout
            {
                return Err(refusal());
            }
            result = Some(serde_json::from_str::<Value>(payload).map_err(|_| refusal())?);
        } else {
            if records.len() == 8 || result.is_some() || zero.is_some() {
                return Err(refusal());
            }
            let record: Record = serde_json::from_str(payload).map_err(|_| refusal())?;
            let index = records.len();
            if record.schema != "conduit.conduitos/compositor-stopped-lifecycle@1"
                || record.part as usize != index
                || record.phase != PHASES[index]
                || record.instance == 0
                || record.identity.make_profile != profile
                || record.identity.make_build != build
                || record.identity.make_image_binding != image
                || record.identity.guest_image_sha256 != guest
                || guest == [0; 32]
                || record.identity.host.is_empty()
                || record.identity.boot.is_empty()
                || record.identity.base.is_empty()
                || record.identity.offer_generation == 0
                || record.identity.width == 0
                || record.identity.height == 0
                || record.staging_prepared_bytes == 0
                || record.cost.reserved_bytes == 0
                || (index != 3 && record.raw_ticks.0.is_none())
                || (index == 3 && record.raw_ticks.0.is_some())
                || record.staging_live_bytes
                    != if matches!(index, 4 | 5) {
                        0
                    } else {
                        record.staging_prepared_bytes
                    }
            {
                return Err(refusal());
            }
            if matches!(index, 2 | 7) {
                let ack = record.ack.0.as_ref().ok_or_else(refusal)?;
                if ack.instance != record.instance || record.namespace.0 != Some(ack.namespace) {
                    return Err(refusal());
                }
                wire::verify_ack(serial, ack, &record.cost).map_err(|_| refusal())?;
            } else if record.ack.0.is_some() || record.namespace.0.is_some() {
                return Err(refusal());
            }
            values.push(serde_json::from_str::<Value>(payload).map_err(|_| refusal())?);
            records.push(record);
        }
    }
    let result = result.ok_or_else(refusal)?;
    if records.len() != 8 {
        return Err(refusal());
    }
    let first = &records[0];
    for (index, record) in records.iter().enumerate() {
        if record.identity != first.identity
            || record.staging_prepared_bytes != first.staging_prepared_bytes
            || record.cost.reserved_bytes != first.cost.reserved_bytes
            || record.instance
                != if index < 6 {
                    first.instance
                } else {
                    records[6].instance
                }
        {
            return Err(refusal());
        }
    }
    if records[6].instance <= first.instance
        || records[0].cost.entries != 0
        || records[1].cost.entries != 1
        || records[2].cost.entries <= records[1].cost.entries
        || records[3].cost.entries <= records[2].cost.entries
        || records[4].cost != records[5].cost
        || records[4].cost.entries != records[3].cost.entries
        || records[4].cost.teardown_zeroed_bytes != records[4].cost.reserved_bytes
        || records[6].cost.teardown_zeroed_bytes != 0
        || records[6].cost.entries != 1
        || records[7].cost.entries <= records[6].cost.entries
    {
        return Err(refusal());
    }
    Ok(
        json!({"records":values,"zero_witness":zero.ok_or_else(refusal)?,"result":result,"measurement_clock":"uncalibrated-raw-tsc","matched_cold_warm_claim":false}),
    )
}

#[cfg(test)]
mod tests;
