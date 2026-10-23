//! Bounded serialization only. Callers choose real checkpoints and write the
//! completed bytes after measurement; this module performs no I/O or clock read.
use super::{Phase, Sample};
use crate::protected_region::DomainCost;
use serde::{Serialize, ser::SerializeStruct};

pub(crate) const CAPACITY: usize = 4096;
pub(crate) const PREFIX: &[u8] = b"CONDUIT_COMPOSITOR_LIFECYCLE ";
const SCHEMA: &str = "conduit.conduitos/compositor-lifecycle@1";

/// Named product checkpoints are not inferred from pictures or frame counts.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Checkpoint {
    Prepared,
    InitialHub,
    FirstCreche,
    PlayingHub,
}
impl Checkpoint {
    pub(crate) const fn ordinal(self) -> u64 {
        match self {
            Self::Prepared => 0,
            Self::InitialHub => 1,
            Self::FirstCreche => 2,
            Self::PlayingHub => 3,
        }
    }
}

/// Correlation to existing validated-copy/guest-ACK telemetry, not scanout authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct AckRef {
    pub instance: u64,
    pub namespace: u64,
    pub frame_sequence: u64,
    pub guest_sequence: u64,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Stage {
    Preparation,
    Activation,
    Request,
    Disposal,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    Context,
    Installed,
    Before,
    After,
    Current,
}

/// Real route/checkpoint selection belongs to the caller/strict collector.
/// Ordinals are observations, not service handles or operation grants.
#[derive(Clone, Copy, Serialize)]
pub(crate) struct Position {
    pub preparation: u64,
    pub checkpoint: u64,
    pub part: u8,
    pub parts: u8,
    pub instance: u64,
    pub namespace: Option<u64>,
}
#[derive(Clone, Copy, Serialize)]
pub(crate) struct Identity<'a> {
    pub host: &'a str,
    pub boot: &'a str,
    pub offer_generation: u64,
    pub base: &'a str,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bits_per_pixel: u8,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
    pub make_profile: &'a str,
    pub make_build: &'a str,
    /// Embedded description binding, NEVER the final ELF or ISO digest.
    pub make_image_binding: &'a str,
    /// Digest of the actual immutable embedded compositor ELF, computed once.
    pub guest_image_sha256: &'a [u8; 32],
}

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
impl<'a> Identity<'a> {
    pub fn borrowed(
        display: crate::protected_compositor::display_grant::DisplayIdentity<'a>,
        make: &'a crate::make::MakeRecord,
        guest_image_sha256: &'a [u8; 32],
    ) -> Self {
        Self {
            host: display.host.as_str(),
            boot: display.boot.as_str(),
            offer_generation: display.generation.0,
            base: display.base.as_str(),
            width: display.format.width,
            height: display.format.height,
            pitch: display.format.pitch,
            bits_per_pixel: display.format.bits_per_pixel,
            red_shift: display.format.red_shift,
            green_shift: display.format.green_shift,
            blue_shift: display.format.blue_shift,
            make_profile: make.profile_id,
            make_build: make.build_id,
            make_image_binding: make.image_binding,
            guest_image_sha256,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
pub(crate) struct SampleMetadata {
    pub succeeded: bool,
    pub refused: bool,
    pub raw_ticks: Option<u64>,
}
impl From<Sample> for SampleMetadata {
    fn from(value: Sample) -> Self {
        Self {
            succeeded: value.succeeded,
            refused: value.refused,
            raw_ticks: value.raw_ticks,
        }
    }
}
#[derive(Clone, Copy, Serialize)]
pub(crate) struct PhaseMetadata {
    pub count: u64,
    pub overflowed: bool,
}
impl From<Phase> for PhaseMetadata {
    fn from(value: Phase) -> Self {
        Self {
            count: value.count,
            overflowed: value.overflowed,
        }
    }
}

/// One numeric cost projection per record keeps the record bound independent
/// of how many cumulative snapshots a checkpoint needs. No computed deltas.
#[derive(Clone, Copy)]
pub(crate) struct Cost<'a>(pub &'a DomainCost);
impl Serialize for Cost<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let c = self.0;
        let mut s = serializer.serialize_struct("DomainCost", 18)?;
        s.serialize_field("entries", &c.entries)?;
        s.serialize_field("interrupt_entries", &c.interrupt_entries)?;
        s.serialize_field("source_timer_interrupts", &c.source_timer_interrupts)?;
        s.serialize_field("privilege_transitions", &c.privilege_transitions)?;
        s.serialize_field("gate_transitions", &c.gate_transitions)?;
        s.serialize_field("copied_bytes", &c.copied_bytes)?;
        s.serialize_field("setup_copied_bytes", &c.setup_copied_bytes)?;
        s.serialize_field("base_gate_transitions", &c.base_gate_transitions)?;
        s.serialize_field("tlb_flushes", &c.tlb_flushes)?;
        s.serialize_field("setup_ticks", &c.setup_ticks)?;
        s.serialize_field("teardown_ticks", &c.teardown_ticks)?;
        s.serialize_field("teardown_zeroed_bytes", &c.teardown_zeroed_bytes)?;
        s.serialize_field("shared_peak_bytes", &c.shared_peak_bytes)?;
        s.serialize_field("root_metadata_bytes", &c.root_metadata_bytes)?;
        s.serialize_field("address_space_switches", &c.address_space_switches)?;
        s.serialize_field("scheduler_returns", &c.scheduler_returns)?;
        s.serialize_field("preemptions", &c.preemptions)?;
        s.serialize_field("reserved_bytes", &c.reserved_bytes)?;
        s.end()
    }
}

#[derive(Serialize)]
pub(crate) struct Snapshot<'a> {
    pub checkpoint: Checkpoint,
    pub ack: Option<AckRef>,
    pub position: Position,
    pub stage: Stage,
    pub kind: Kind,
    pub identity: Identity<'a>,
    pub phase: Option<PhaseMetadata>,
    pub sample: Option<SampleMetadata>,
    pub cost: Option<Cost<'a>>,
    pub staging_prepared_bytes: u64,
    pub staging_live_bytes: u64,
    pub installation_raw_ticks: Option<u64>,
    pub staging_preparation_raw_ticks: Option<u64>,
    pub preparation_raw_ticks: Option<u64>,
    pub disposal_raw_ticks: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum RecordKind {
    Snapshot,
    Incomplete,
}

#[derive(Serialize)]
struct Record<'a> {
    record_kind: RecordKind,
    schema: &'static str,
    complete: bool,
    snapshot: &'a Snapshot<'a>,
}
#[derive(Serialize)]
struct Incomplete {
    record_kind: RecordKind,
    schema: &'static str,
    complete: bool,
    position: Position,
    stage: Stage,
    reason: &'static str,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Encoded {
    /// Whole record including prefix and final newline. No partial bytes escape.
    pub length: usize,
    pub complete: bool,
}

/// Never returns a product error. If a record cannot fit, replace every partial
/// byte with a small explicit incomplete record. Even unavailable counters are
/// null; a counter overflow remains explicit and makes the evidence incomplete.
pub(crate) fn encode(snapshot: &Snapshot<'_>, buffer: &mut [u8; CAPACITY]) -> Encoded {
    buffer.fill(0);
    let overflowed = snapshot.phase.is_some_and(|phase| phase.overflowed);
    if !overflowed {
        let record = Record {
            record_kind: RecordKind::Snapshot,
            schema: SCHEMA,
            complete: true,
            snapshot,
        };
        if let Ok(length) =
            serde_json_core::to_slice(&record, &mut buffer[PREFIX.len()..CAPACITY - 1])
        {
            buffer[..PREFIX.len()].copy_from_slice(PREFIX);
            let end = PREFIX.len() + length;
            buffer[end] = b'\n';
            return Encoded {
                length: end + 1,
                complete: true,
            };
        }
    }
    encode_failure(
        snapshot.position,
        snapshot.stage,
        if overflowed {
            "counter-overflow"
        } else {
            "record-capacity-or-serialization"
        },
        buffer,
    )
}

/// A selected route returned without an actual qualifying ACK. Receipt loss
/// is explicit and must never change the product operation's result.
pub(crate) fn encode_missing_ack(position: Position, buffer: &mut [u8; CAPACITY]) -> Encoded {
    encode_failure(
        position,
        Stage::Activation,
        "missing-ack-or-route-refused",
        buffer,
    )
}
fn encode_failure(
    position: Position,
    stage: Stage,
    reason: &'static str,
    buffer: &mut [u8; CAPACITY],
) -> Encoded {
    buffer.fill(0);
    let record = Incomplete {
        record_kind: RecordKind::Incomplete,
        schema: SCHEMA,
        complete: false,
        position,
        stage,
        reason,
    };
    // All fallback fields are closed enums/integers/static strings; tests cover
    // maximum values. Keep a static valid marker even if the serializer changes.
    let length = match serde_json_core::to_slice(&record, &mut buffer[PREFIX.len()..CAPACITY - 1]) {
        Ok(length) => length,
        Err(_) => {
            const FALLBACK: &[u8] = b"{\"record_kind\":\"incomplete\",\"schema\":\"conduit.conduitos/compositor-lifecycle@1\",\"complete\":false,\"reason\":\"incomplete-marker-serialization\"}";
            buffer[PREFIX.len()..PREFIX.len() + FALLBACK.len()].copy_from_slice(FALLBACK);
            FALLBACK.len()
        }
    };
    buffer[..PREFIX.len()].copy_from_slice(PREFIX);
    let end = PREFIX.len() + length;
    buffer[end] = b'\n';
    Encoded {
        length: end + 1,
        complete: false,
    }
}

#[cfg(test)]
#[path = "lifecycle_sign_tests.rs"]
mod tests;
