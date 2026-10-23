//! Three matched render/copy pairs; control costs are measured, not forced equal.
use super::super::{
    compositor_lifecycle_proof::{self as wire, AckRef, Cost, Identity, Nullable},
    ConduitosError,
};
use serde::Deserialize;
use serde_json::{json, Value};
const PREFIX: &str = "CONDUIT_COMPOSITOR_RETAINED_PAIR ";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    trial: u8,
    arm: String,
    identity: Identity,
    instance: u64,
    ack: Nullable<AckRef>,
    before: Cost,
    after: Cost,
    elapsed_raw_ticks: Nullable<u64>,
    installation_raw_ticks: Nullable<u64>,
    initialization_raw_ticks: Nullable<u64>,
    activation_raw_ticks: Nullable<u64>,
    activation_entries: Nullable<u64>,
    face: String,
    scene_commands: usize,
    raster_commands: u8,
    raster_pixels: u32,
    target_writes: u32,
    scene_equal_within_pair: bool,
    zero_audit_armed: bool,
    wall_nanoseconds: Nullable<u64>,
}
fn refusal() -> ConduitosError {
    ConduitosError::refusal(
        "compositor-retained-pair-mismatch",
        "requires three actual same-Face/Scene/raster/framebuffer-copy pairs with retained instance, fresh namespace, unchanged installation/resource custody and real ACKs; control costs need not be equal",
    )
}
fn delta(before: &Cost, after: &Cost) -> Result<Value, ConduitosError> {
    Ok(
        json!({"entries":after.entries.checked_sub(before.entries).ok_or_else(refusal)?,
        "copied_bytes":after.copied_bytes.checked_sub(before.copied_bytes).ok_or_else(refusal)?,
        "privilege_transitions":after.privilege_transitions.checked_sub(before.privilege_transitions).ok_or_else(refusal)?,
        "address_space_switches":after.address_space_switches.checked_sub(before.address_space_switches).ok_or_else(refusal)?,
        "scheduler_returns":after.scheduler_returns.checked_sub(before.scheduler_returns).ok_or_else(refusal)?}),
    )
}
fn frame_work(serial: &str, ack: &AckRef, cost: &Cost) -> Result<[u64; 3], ConduitosError> {
    // Existing strict decoder validates every frame line and the exact final
    // ACK/counter tuple. Only then aggregate actual copy/damage telemetry for
    // this unique instance/namespace, including both present compositions.
    wire::verify_ack(serial, ack, cost).map_err(|_| refusal())?;
    let mut totals = [0_u64; 3];
    for line in serial
        .lines()
        .filter(|line| line.starts_with("CONDUIT_COMPOSITOR_SERVICE_FRAME "))
    {
        let value = |key| -> Result<u64, ConduitosError> {
            line.split_whitespace()
                .filter_map(|field| field.split_once('='))
                .find(|(name, _)| *name == key)
                .ok_or_else(refusal)?
                .1
                .parse()
                .map_err(|_| refusal())
        };
        if value("service_instance")? == ack.instance && value("namespace")? == ack.namespace {
            for (total, add) in
                totals
                    .iter_mut()
                    .zip([1, value("pixels_written")?, value("damage_count")?])
            {
                *total = total.checked_add(add).ok_or_else(refusal)?;
            }
        }
    }
    if totals.contains(&0) {
        return Err(refusal());
    }
    Ok(totals)
}
fn same_resources(a: &Cost, b: &Cost) -> bool {
    a.setup_ticks == b.setup_ticks
        && a.setup_copied_bytes == b.setup_copied_bytes
        && a.reserved_bytes == b.reserved_bytes
        && a.root_metadata_bytes == b.root_metadata_bytes
        && a.teardown_zeroed_bytes == 0
        && b.teardown_zeroed_bytes == 0
}
pub(super) fn validate(
    serial: &str,
    profile: &str,
    build: &str,
    image: &str,
    guest: [u8; 32],
) -> Result<Value, ConduitosError> {
    let mut records: Vec<Record> = Vec::with_capacity(9);
    let mut values = Vec::with_capacity(9);
    for line in serial
        .split_inclusive('\n')
        .filter(|line| line.starts_with(PREFIX.trim_end()))
    {
        if records.len() == 9 || line.len() > 4096 || !line.ends_with('\n') {
            return Err(refusal());
        }
        let payload = line.strip_prefix(PREFIX).ok_or_else(refusal)?;
        let record: Record = serde_json::from_str(payload).map_err(|_| refusal())?;
        let index = records.len();
        let prepared = index % 3 == 0;
        if record.schema != "conduit.conduitos/compositor-retained-pair@1"
            || record.trial as usize != index / 3
            || record.arm != ["prepared", "cold", "retained"][index % 3]
            || record.instance == 0
            || record.identity.make_profile != profile
            || record.identity.make_build != build
            || record.identity.make_image_binding != image
            || record.identity.guest_image_sha256 != guest
            || guest == [0; 32]
            || record.identity.host.is_empty()
            || record.identity.boot.is_empty()
            || record.identity.base.is_empty()
            || record.identity.width == 0
            || record.identity.height == 0
            || record.identity.offer_generation == 0
            || record.face.is_empty()
            || record.scene_commands == 0
            || record.scene_commands > 48
            || record.zero_audit_armed
            || !record.scene_equal_within_pair
            || record.wall_nanoseconds.0.is_some()
            || record.elapsed_raw_ticks.0.is_none()
            || !same_resources(&record.before, &record.after)
        {
            return Err(refusal());
        }
        if prepared {
            if record.ack.0.is_some()
                || record.installation_raw_ticks.0.is_none()
                || record.initialization_raw_ticks.0.is_none()
                || record.activation_raw_ticks.0.is_some()
                || record.activation_entries.0.is_some()
                || record.before.entries != 0
                || record.after.entries != 1
                || record.after.reserved_bytes == 0
                || record.target_writes != 0
                || record.raster_commands != 0
                || record.raster_pixels != 0
            {
                return Err(refusal());
            }
        } else {
            let ack = record.ack.0.as_ref().ok_or_else(refusal)?;
            if ack.instance != record.instance
                || record.installation_raw_ticks.0.is_some()
                || record.initialization_raw_ticks.0.is_some()
                || record.activation_raw_ticks.0.is_none()
                || record
                    .activation_entries
                    .0
                    .is_none_or(|entries| entries == 0)
                || record.after.entries <= record.before.entries
                || record.raster_commands == 0
                || record.raster_pixels == 0
                || record.target_writes == 0
                || frame_work(serial, ack, &record.after)?[1] != u64::from(record.target_writes)
            {
                return Err(refusal());
            }
            delta(&record.before, &record.after)?;
        }
        if let Some(first) = records.first() {
            if record.identity != first.identity
                || record.face != first.face
                || record.scene_commands != first.scene_commands
            {
                return Err(refusal());
            }
        }
        values.push(serde_json::from_str::<Value>(payload).map_err(|_| refusal())?);
        records.push(record);
    }
    if records.len() != 9 {
        return Err(refusal());
    }
    let mut pairs = Vec::with_capacity(3);
    for (trial, batch) in records.chunks_exact(3).enumerate() {
        let [prepared, cold, retained] = batch else {
            return Err(refusal());
        };
        let cold_ack = cold.ack.0.as_ref().ok_or_else(refusal)?;
        let warm_ack = retained.ack.0.as_ref().ok_or_else(refusal)?;
        if cold.instance != prepared.instance
            || retained.instance != prepared.instance
            || cold.before != prepared.after
            || warm_ack.namespace <= cold_ack.namespace
            || retained.before.entries <= cold.after.entries
            || cold.raster_commands != retained.raster_commands
            || cold.raster_pixels != retained.raster_pixels
            || cold.target_writes != retained.target_writes
            || !same_resources(&prepared.after, &retained.after)
            || frame_work(serial, cold_ack, &cold.after)?
                != frame_work(serial, warm_ack, &retained.after)?
            || (trial > 0 && prepared.instance <= records[(trial - 1) * 3].instance)
        {
            return Err(refusal());
        }
        pairs.push(json!({"trial":trial,"instance":prepared.instance,"cold_control_cost":delta(&cold.before,&cold.after)?,
            "retained_control_cost":delta(&retained.before,&retained.after)?,"copy_work":frame_work(serial,cold_ack,&cold.after)?,
            "cold_raw_ticks":cold.elapsed_raw_ticks.0,"retained_raw_ticks":retained.elapsed_raw_ticks.0}));
    }
    let cold_ticks = records
        .chunks_exact(3)
        .map(|batch| batch[1].elapsed_raw_ticks.0)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(refusal)?;
    let retained_ticks = records
        .chunks_exact(3)
        .map(|batch| batch[2].elapsed_raw_ticks.0)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(refusal)?;
    Ok(
        json!({"schema":"conduit.conduitos/compositor-retained-pairs-evidence@1","records":values,"pairs":pairs,
        "trials":3,"matched_scope":"same-face-scene-raster-and-framebuffer-copy-work",
        "request_stream_equality_claim":false,"control_costs_forced_equal":false,"zero_audit_overhead":false,
        "observer":"existing-lifecycle-snapshots-and-frame-write-counts","clock":"uncalibrated-raw-tsc","wall_time_available":false,
        "withdrawal_outside_present_intervals":true,"causal_speedup_claim":false,
        "cold_definition":"fresh-protected-service-instance; CPU caches are not flushed",
        "presentation_interval":"entire NativeFaceMask::present including existing frame serial telemetry",
        "includes_existing_frame_serial_telemetry":true,
        "cold_presentation_raw_ticks":cold_ticks,"retained_presentation_raw_ticks":retained_ticks,
        "cold_spread_raw_ticks":{"min":cold_ticks.iter().min(),"max":cold_ticks.iter().max()},
        "retained_spread_raw_ticks":{"min":retained_ticks.iter().min(),"max":retained_ticks.iter().max()}}),
    )
}
#[cfg(test)]
mod tests;
