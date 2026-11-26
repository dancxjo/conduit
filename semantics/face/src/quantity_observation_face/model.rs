//! Typed projection inputs, not a decoder, schema escape or producer authority.
use crate::*;
use core::mem::size_of;
pub const MAX_REPORTED_QUANTITY_OBSERVATIONS: usize = 64;
#[derive(Clone, Copy)]
pub enum ReportedQuantityOutcome<'a> {
    Available(&'a conduit_core::Quantity),
    Unavailable(&'a str),
}
#[derive(Clone, Copy)]
pub struct ReportedQuantityObservation<'a> {
    pub sequence: u64,
    pub sampled_at_boot_ms: u64,
    pub outcome: ReportedQuantityOutcome<'a>,
    pub gap_before: Option<(u64, u64)>,
}
/// Explicit reported recovery accounting. Unknown preceding history must remain
/// unknown; counts describe accepted samples, never missing sequence positions.
#[derive(Clone, Copy)]
pub struct ReportedQuantityRecovery {
    pub chunks: u16,
    pub samples: u32,
    pub preceding_history_known: bool,
}
#[derive(Clone, Copy)]
pub struct ReportedQuantityChannel<'a> {
    pub source_identity: &'a str,
    pub source_name: &'a str,
    pub channel_identity: &'a str,
    pub channel_name: &'a str,
    pub producer: [&'a str; 4],
    pub samples_outside_view: u64,
    pub recovery: Option<ReportedQuantityRecovery>,
}
/// Requested backing for one admitted channel, including inline fragment and
/// exact vector capacities. It excludes caller-owned samples/quantities, schemas and input
/// codec workspace, allocator bookkeeping and stack. No Step confinement claim.
pub fn reported_quantity_fragment_backing_bound(
    maximum_samples: usize,
    recovery: bool,
) -> Option<usize> {
    if maximum_samples > MAX_REPORTED_QUANTITY_OBSERVATIONS {
        return None;
    }
    let n = maximum_samples;
    let properties = 11 + 7 * n + if recovery { 3 } else { 0 };
    Some(
        size_of::<FaceFragment>()
            + 3 * MAX_FACE_ID_BYTES
            + (n + 2) * (size_of::<FaceSubject>() + 156 + 128 + QUANTITY_SERIES_ROLE.len())
            + 2 * n * (size_of::<FaceRelationship>() + 2 * 156)
            + properties
                * (size_of::<FaceProperty>()
                    + 156
                    + 32
                    + conduit_core::QUANTITY_ENCODED_LEN
                    + 1024),
    )
}
/// Two channels can coexist with aggregate vectors and a retained gate clone.
/// Three aggregate envelopes conservatively cover that preparation overlap.
/// Caller adds original typed rows and their separately checked domain decoder.
pub fn reported_quantity_projection_workspace_bytes(
    maximum_samples: usize,
    channels: usize,
    recovery: bool,
) -> Option<usize> {
    if !(1..=2).contains(&channels) {
        return None;
    }
    let channel = reported_quantity_fragment_backing_bound(maximum_samples, recovery)?;
    let label = size_of::<FaceProperty>() + 128 + "inspection-label".len() + 128;
    channel
        .checked_add(label)?
        .checked_mul(channels)?
        .checked_mul(3)
}
