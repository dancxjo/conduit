//! Resumable, schema-specific CAPSEQ1 graph encoder. No retained model borrow.
use super::Refusal;
#[path = "outer.rs"]
mod outer;
use outer::{Node, Shape};
mod engine;
pub(crate) use engine::GraphEncoding;
pub(crate) const MAX_DEPTH: usize = 128;
pub(crate) const STEP_WORK: u32 = 4096;
// Includes at most 128 indexed path edges and one constant-width physical encoding.
const RESOLVE_RESERVE: u32 = 2048;
// Every visited node/key/container step contributes output syntax; source bytes
// contribute at least as many output bytes. Each resolution costs <= reserve.
pub(crate) const MAX_WORK: u64 =
    (super::MAX_DECODED_JSON_BYTES as u64) * (RESOLVE_RESERVE as u64 * 4 + 16);
pub(crate) const MAX_TURNS: u32 = (MAX_WORK / (STEP_WORK - RESOLVE_RESERVE) as u64 + 2) as u32;

#[cfg(test)]
mod nested_oracle_tests;

#[cfg(test)]
mod graph_oracle_tests;
