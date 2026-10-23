//! Serialize after all trials; no serial work occurs inside presentation intervals.
use super::*;
use crate::protected_compositor::lifecycle::sign::{Cost, Identity};
use serde::Serialize;
#[derive(Serialize)]
struct Record<'a> {
    schema: &'static str,
    trial: u8,
    arm: &'static str,
    identity: Identity<'a>,
    instance: u64,
    ack: Option<AckRef>,
    before: Cost<'a>,
    after: Cost<'a>,
    elapsed_raw_ticks: Option<u64>,
    installation_raw_ticks: Option<u64>,
    initialization_raw_ticks: Option<u64>,
    activation_raw_ticks: Option<u64>,
    activation_entries: Option<u64>,
    face: &'a str,
    scene_commands: usize,
    raster_commands: u8,
    raster_pixels: u32,
    target_writes: u32,
    scene_equal_within_pair: bool,
    zero_audit_armed: bool,
    wall_nanoseconds: Option<u64>,
}
pub(super) fn emit(grant: &SharedDisplayGrant, trials: &[Trial; 3]) {
    let mut buffer = [0_u8; 4096];
    for (index, trial) in trials.iter().enumerate() {
        let prepared = trial.prepared;
        for kind in 0..3 {
            let arm = if kind == 1 {
                &trial.cold
            } else {
                &trial.retained
            };
            let activation = arm.after.activations.attempts.last;
            let activation_entries =
                activation.and_then(|s| s.after?.entries.checked_sub(s.before?.entries));
            let record = Record {
                schema: "conduit.conduitos/compositor-retained-pair@1",
                trial: index as u8,
                arm: ["prepared", "cold", "retained"][kind],
                identity: Identity::borrowed(
                    grant.identity(),
                    &crate::make::EMBEDDED_MAKE,
                    &prepared.guest_image_sha256,
                ),
                instance: prepared.service.instance,
                ack: (kind != 0).then_some(arm.ack),
                before: Cost(if kind == 0 {
                    &prepared.service.installed
                } else {
                    &arm.before.current_cost
                }),
                after: Cost(if kind == 0 {
                    &prepared.current_cost
                } else {
                    &arm.after.current_cost
                }),
                elapsed_raw_ticks: if kind == 0 {
                    prepared.preparation_raw_ticks
                } else {
                    arm.elapsed_raw_ticks
                },
                installation_raw_ticks: (kind == 0)
                    .then_some(prepared.service.installation_raw_ticks)
                    .flatten(),
                initialization_raw_ticks: if kind == 0 {
                    prepared.service.initialization.and_then(|s| s.raw_ticks)
                } else {
                    None
                },
                activation_raw_ticks: if kind == 0 {
                    None
                } else {
                    activation.and_then(|s| s.raw_ticks)
                },
                activation_entries: if kind == 0 { None } else { activation_entries },
                face: trial.face.as_str(),
                scene_commands: trial.scene.commands().len(),
                raster_commands: if kind == 0 { 0 } else { arm.raster.commands },
                raster_pixels: if kind == 0 {
                    0
                } else {
                    arm.raster.pixels_written
                },
                target_writes: if kind == 0 { 0 } else { arm.writes },
                scene_equal_within_pair: true,
                zero_audit_armed: false,
                wall_nanoseconds: None,
            };
            let length = serde_json_core::to_slice(&record, &mut buffer)
                .unwrap_or_else(|_| refuse("matched-record-bound"));
            const PREFIX: &[u8] = b"CONDUIT_COMPOSITOR_RETAINED_PAIR ";
            if length + PREFIX.len() + 1 > buffer.len() {
                refuse("matched-line-bound");
            }
            arch::early_write(PREFIX);
            arch::early_write(&buffer[..length]);
            arch::early_write(b"\n");
        }
    }
}
