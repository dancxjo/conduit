//! Wire types: even nullable fields are required, and unknown fields refuse.
use super::std::{string::String, vec::Vec};
use serde::{Deserialize, Deserializer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nullable<T>(pub Option<T>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Decoding Option directly also accepts Serde's missing-field sentinel.
        // Buffering this closed JSON value requires a real field first, while
        // retaining explicit null as a distinct permitted wire value.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Present<T> {
            Null(()),
            Value(T),
        }
        Present::<T>::deserialize(deserializer).map(|value| {
            Self(match value {
                Present::Null(()) => None,
                Present::Value(value) => Some(value),
            })
        })
    }
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Checkpoint {
    Prepared,
    InitialHub,
    FirstCreche,
    PlayingHub,
}
impl Checkpoint {
    pub fn ordinal(self) -> usize {
        match self {
            Self::Prepared => 0,
            Self::InitialHub => 1,
            Self::FirstCreche => 2,
            Self::PlayingHub => 3,
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    Preparation,
    Activation,
    Request,
    Disposal,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Context,
    Installed,
    Before,
    After,
    Current,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AckRef {
    pub instance: u64,
    pub namespace: u64,
    pub frame_sequence: u64,
    pub guest_sequence: u64,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub preparation: u64,
    pub checkpoint: u64,
    pub part: u8,
    pub parts: u8,
    pub instance: u64,
    pub namespace: Nullable<u64>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub host: String,
    pub boot: String,
    pub offer_generation: u64,
    pub base: String,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bits_per_pixel: u8,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
    pub make_profile: String,
    pub make_build: String,
    pub make_image_binding: String,
    pub guest_image_sha256: [u8; 32],
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub count: u64,
    pub overflowed: bool,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub succeeded: bool,
    pub refused: bool,
    pub raw_ticks: Nullable<u64>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub entries: u64,
    pub interrupt_entries: u64,
    pub source_timer_interrupts: u64,
    pub privilege_transitions: u64,
    pub gate_transitions: u64,
    pub copied_bytes: u64,
    pub setup_copied_bytes: u64,
    pub base_gate_transitions: u64,
    pub tlb_flushes: u64,
    pub setup_ticks: u64,
    pub teardown_ticks: u64,
    pub teardown_zeroed_bytes: u32,
    pub shared_peak_bytes: u32,
    pub root_metadata_bytes: u32,
    pub address_space_switches: u64,
    pub scheduler_returns: u64,
    pub preemptions: u64,
    pub reserved_bytes: u32,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub position: Position,
    pub checkpoint: Checkpoint,
    pub ack: Nullable<AckRef>,
    pub stage: Stage,
    pub kind: Kind,
    pub identity: Identity,
    pub phase: Nullable<Phase>,
    pub sample: Nullable<Sample>,
    pub cost: Nullable<Cost>,
    pub staging_prepared_bytes: u64,
    pub staging_live_bytes: u64,
    pub installation_raw_ticks: Nullable<u64>,
    pub staging_preparation_raw_ticks: Nullable<u64>,
    pub preparation_raw_ticks: Nullable<u64>,
    pub disposal_raw_ticks: Nullable<u64>,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
// Keep the bounded parsed record inline; boxing changes allocation semantics
// solely to reduce this private, short-lived discriminator's stack size.
#[allow(clippy::large_enum_variant)]
pub(super) enum Packet {
    Snapshot(SnapshotRecord),
    Incomplete(IncompleteRecord),
    Emergency(EmergencyRecord),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SnapshotRecord {
    pub record_kind: SnapshotKind,
    pub schema: String,
    pub complete: bool,
    pub snapshot: Snapshot,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SnapshotKind {
    Snapshot,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum IncompleteKind {
    Incomplete,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IncompleteReason {
    CounterOverflow,
    RecordCapacityOrSerialization,
    IncompleteMarkerSerialization,
    MissingAckOrRouteRefused,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IncompleteRecord {
    pub record_kind: IncompleteKind,
    pub schema: String,
    pub complete: bool,
    pub position: Position,
    pub stage: Stage,
    pub reason: IncompleteReason,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmergencyRecord {
    pub record_kind: IncompleteKind,
    pub schema: String,
    pub complete: bool,
    pub reason: IncompleteReason,
}

/// Caller-provided expected values. This module never reads or verifies artifacts.
#[derive(Clone, Debug)]
pub struct ExpectedBinding {
    pub source_commit: String,
    pub root_elf_sha256: String,
    pub iso_sha256: String,
    pub identity: Identity,
    pub preparation: u64,
    pub service_reserved_bytes: u32,
    pub staging_prepared_bytes: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Oversize,
    Truncated,
    Malformed,
    UnknownSchema,
    Incomplete(IncompleteReason),
    RecordCount,
    BatchShape,
    Scope,
    Metadata,
    Counter,
    Ack,
    Frame,
}
#[derive(Debug)]
pub struct CheckpointReport {
    pub checkpoint: Checkpoint,
    pub ack: Option<AckRef>,
    pub activation_count: Option<u64>,
    pub request_count: Option<u64>,
}
#[derive(Debug)]
pub struct Report {
    pub source_commit: String,
    pub root_elf_sha256: String,
    pub iso_sha256: String,
    pub instance: u64,
    pub checkpoints: Vec<CheckpointReport>,
    pub lifecycle_records: usize,
    /// Always false: caller must separately verify actual sealed artifact bytes.
    pub artifact_custody_verified: bool,
}
