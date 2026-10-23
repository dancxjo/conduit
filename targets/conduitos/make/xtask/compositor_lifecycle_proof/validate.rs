//! Fixed selected checkpoint policy, separate from line decoding and artifact I/O.
use super::{frame::Frame, *};

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn counters(cost: &Cost) -> [u64; 11] {
    // Setup envelopes, reserved bytes, metadata, peaks and live staging are NOT
    // interchangeable cumulative counters. Do not subtract all numeric fields.
    [
        cost.entries,
        cost.interrupt_entries,
        cost.source_timer_interrupts,
        cost.privilege_transitions,
        cost.gate_transitions,
        cost.copied_bytes,
        cost.base_gate_transitions,
        cost.tlb_flushes,
        cost.address_space_switches,
        cost.scheduler_returns,
        cost.preemptions,
    ]
}
fn forward(before: &Cost, after: &Cost) -> Result<(), Refusal> {
    if counters(before)
        .iter()
        .zip(counters(after))
        .any(|(a, b)| b.checked_sub(*a).is_none())
    {
        return Err(Refusal::Counter);
    }
    Ok(())
}
fn metadata(a: &Snapshot, b: &Snapshot) -> bool {
    a.identity == b.identity
        && a.checkpoint == b.checkpoint
        && a.ack == b.ack
        && a.position.preparation == b.position.preparation
        && a.position.checkpoint == b.position.checkpoint
        && a.position.instance == b.position.instance
        && a.position.namespace == b.position.namespace
        && a.position.parts == b.position.parts
        && a.staging_prepared_bytes == b.staging_prepared_bytes
        && a.staging_live_bytes == b.staging_live_bytes
        && a.installation_raw_ticks == b.installation_raw_ticks
        && a.staging_preparation_raw_ticks == b.staging_preparation_raw_ticks
        && a.preparation_raw_ticks == b.preparation_raw_ticks
        && a.disposal_raw_ticks == b.disposal_raw_ticks
}
fn paired(before: &Snapshot, after: &Snapshot, prepared: bool) -> Result<(), Refusal> {
    if before.sample != after.sample || before.phase != after.phase {
        return Err(Refusal::Metadata);
    }
    let sample = before.sample.0.as_ref().ok_or(Refusal::Metadata)?;
    if !sample.succeeded || sample.refused {
        return Err(Refusal::Metadata);
    }
    if prepared {
        if before.phase.0.is_some() {
            return Err(Refusal::Metadata);
        }
    } else {
        let phase = before.phase.0.as_ref().ok_or(Refusal::Metadata)?;
        if phase.count == 0 || phase.overflowed {
            return Err(Refusal::Metadata);
        }
    }
    forward(
        before.cost.0.as_ref().ok_or(Refusal::BatchShape)?,
        after.cost.0.as_ref().ok_or(Refusal::BatchShape)?,
    )
}

pub(super) fn validate(
    records: &[Snapshot],
    frames: &[Frame],
    expected: &ExpectedBinding,
) -> Result<Report, Refusal> {
    if !hex(&expected.source_commit, 40)
        || !hex(&expected.root_elf_sha256, 64)
        || !hex(&expected.iso_sha256, 64)
        || expected.preparation == 0
        || expected.service_reserved_bytes == 0
        || expected.staging_prepared_bytes == 0
    {
        return Err(Refusal::Scope);
    }
    let identity = &expected.identity;
    for value in [
        &identity.host,
        &identity.boot,
        &identity.base,
        &identity.make_profile,
        &identity.make_build,
        &identity.make_image_binding,
    ] {
        if value.is_empty() || value.len() > 512 {
            return Err(Refusal::Scope);
        }
    }
    if records.len() != 23 {
        return Err(Refusal::RecordCount);
    }
    let instance = records[0].position.instance;
    if instance == 0 {
        return Err(Refusal::Scope);
    }
    let mut offset = 0;
    let mut namespace = 0;
    let mut previous_counts = (0, 0);
    let mut reports = Vec::with_capacity(4);
    for checkpoint in [
        Checkpoint::Prepared,
        Checkpoint::InitialHub,
        Checkpoint::FirstCreche,
        Checkpoint::PlayingHub,
    ] {
        let prepared = checkpoint == Checkpoint::Prepared;
        let count = if prepared { 5 } else { 6 };
        let batch = &records[offset..offset + count];
        let first = &batch[0];
        for (part, value) in batch.iter().enumerate() {
            if value.checkpoint != checkpoint
                || value.position.checkpoint != checkpoint.ordinal() as u64
                || value.position.part != part as u8
                || value.position.parts != count as u8
                || !metadata(first, value)
            {
                return Err(Refusal::BatchShape);
            }
            if value.position.instance != instance
                || value.position.preparation != expected.preparation
                || value.identity != expected.identity
                || value.staging_prepared_bytes != expected.staging_prepared_bytes
                || value.staging_live_bytes != expected.staging_prepared_bytes
            {
                return Err(Refusal::Scope);
            }
            if value.disposal_raw_ticks.0.is_some()
                || value.phase.0.as_ref().is_some_and(|phase| phase.overflowed)
            {
                return Err(Refusal::Metadata);
            }
            if value
                .cost
                .0
                .as_ref()
                .is_some_and(|cost| cost.reserved_bytes != expected.service_reserved_bytes)
            {
                return Err(Refusal::Scope);
            }
        }
        let shape: &[(Stage, Kind)] = if prepared {
            &[
                (Stage::Preparation, Kind::Context),
                (Stage::Preparation, Kind::Installed),
                (Stage::Preparation, Kind::Before),
                (Stage::Preparation, Kind::After),
                (Stage::Preparation, Kind::Current),
            ]
        } else {
            &[
                (Stage::Activation, Kind::Context),
                (Stage::Activation, Kind::Before),
                (Stage::Activation, Kind::After),
                (Stage::Request, Kind::Before),
                (Stage::Request, Kind::After),
                (Stage::Request, Kind::Current),
            ]
        };
        for (value, (stage, kind)) in batch.iter().zip(shape) {
            if value.stage != *stage || value.kind != *kind {
                return Err(Refusal::BatchShape);
            }
            match kind {
                Kind::Context => {
                    if value.cost.0.is_some() || value.sample.0.is_some() || value.phase.0.is_some()
                    {
                        return Err(Refusal::BatchShape);
                    }
                }
                Kind::Current | Kind::Installed => {
                    if value.cost.0.is_none() || value.sample.0.is_some() || value.phase.0.is_some()
                    {
                        return Err(Refusal::BatchShape);
                    }
                }
                Kind::Before | Kind::After => {
                    if value.cost.0.is_none() {
                        return Err(Refusal::BatchShape);
                    }
                }
            }
        }
        let (activation_count, request_count) = if prepared {
            if first.ack.0.is_some() || first.position.namespace.0.is_some() {
                return Err(Refusal::Ack);
            }
            paired(&batch[2], &batch[3], true)?;
            forward(
                batch[1].cost.0.as_ref().unwrap(),
                batch[2].cost.0.as_ref().unwrap(),
            )?;
            forward(
                batch[3].cost.0.as_ref().unwrap(),
                batch[4].cost.0.as_ref().unwrap(),
            )?;
            (None, None)
        } else {
            let ack = first.ack.0.ok_or(Refusal::Ack)?;
            if ack.instance != instance
                || ack.namespace <= namespace
                || first.position.namespace.0 != Some(ack.namespace)
                || ack.frame_sequence == 0
                || ack.guest_sequence == 0
            {
                return Err(Refusal::Ack);
            }
            let matches: Vec<_> = frames.iter().filter(|frame| frame.ack == ack).collect();
            if matches.len() != 1 || matches[0].preceding_records > offset {
                return Err(Refusal::Ack);
            }
            let current = batch[5].cost.0.as_ref().unwrap();
            if matches[0].counters
                != [
                    current.entries,
                    current.privilege_transitions,
                    current.address_space_switches,
                    current.scheduler_returns,
                ]
                || batch[4].cost != batch[5].cost
            {
                return Err(Refusal::Ack);
            }
            namespace = ack.namespace;
            paired(&batch[1], &batch[2], false)?;
            paired(&batch[3], &batch[4], false)?;
            forward(
                batch[2].cost.0.as_ref().unwrap(),
                batch[3].cost.0.as_ref().unwrap(),
            )?;
            forward(
                batch[4].cost.0.as_ref().unwrap(),
                batch[5].cost.0.as_ref().unwrap(),
            )?;
            (
                Some(batch[1].phase.0.as_ref().unwrap().count),
                Some(batch[3].phase.0.as_ref().unwrap().count),
            )
        };
        if let (Some(a), Some(r)) = (activation_count, request_count) {
            if a <= previous_counts.0 || r <= previous_counts.1 {
                return Err(Refusal::Counter);
            }
            previous_counts = (a, r);
        }
        reports.push(CheckpointReport {
            checkpoint,
            ack: first.ack.0,
            activation_count,
            request_count,
        });
        offset += count;
    }
    Ok(Report {
        source_commit: expected.source_commit.clone(),
        root_elf_sha256: expected.root_elf_sha256.clone(),
        iso_sha256: expected.iso_sha256.clone(),
        instance,
        checkpoints: reports,
        lifecycle_records: records.len(),
        artifact_custody_verified: false,
    })
}
