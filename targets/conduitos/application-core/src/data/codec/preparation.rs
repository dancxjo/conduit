//! Complete retained graphs cross the private boundary in finite binary form.
//! Graph custody JSON and every ordinary graph constructor remain unchanged.
use super::{Command, Refusal, finite_cbor};
use crate::{data::Target, graph_facts::Scope};
use patchbay_graph::PatchbayGraph;
use serde::{Deserialize, Serialize};
const MAGIC: &[u8] = b"CAPG\x01";

#[derive(Serialize)]
enum Borrowed<'a> {
    PrepareTarget {
        slot: u8,
        target: u8,
        value: &'a Target,
    },
    PrepareGraphProjection {
        graph: &'a PatchbayGraph,
        scope: &'a Scope,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum Owned {
    PrepareTarget { slot: u8, target: u8, value: Target },
    PrepareGraphProjection { graph: PatchbayGraph, scope: Scope },
}
pub(super) fn matches(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
pub(super) fn encode(command: &Command, output: &mut [u8]) -> Result<usize, Refusal> {
    let value = match command {
        Command::PrepareTarget {
            slot,
            target,
            value,
        } => Borrowed::PrepareTarget {
            slot: *slot,
            target: *target,
            value,
        },
        Command::PrepareGraphProjection { graph, scope } => {
            Borrowed::PrepareGraphProjection { graph, scope }
        }
        _ => return Err(Refusal::Order),
    };
    output
        .get_mut(..MAGIC.len())
        .ok_or(Refusal::Capacity)?
        .copy_from_slice(MAGIC);
    let length = MAGIC.len() + finite_cbor::encode(&value, &mut output[MAGIC.len()..])?;
    preflight(&output[..length])?;
    Ok(length)
}
pub(super) fn preflight(bytes: &[u8]) -> Result<(), Refusal> {
    finite_cbor::validate(
        bytes.strip_prefix(MAGIC).ok_or(Refusal::Malformed)?,
        super::MAX_DECODED_JSON_BYTES,
    )
}
pub(super) fn decode_preflighted(bytes: &[u8]) -> Result<Command, Refusal> {
    Ok(
        match finite_cbor::decode_preflighted(bytes.strip_prefix(MAGIC).ok_or(Refusal::Malformed)?)?
        {
            Owned::PrepareTarget {
                slot,
                target,
                value,
            } => Command::PrepareTarget {
                slot,
                target,
                value,
            },
            Owned::PrepareGraphProjection { graph, scope } => {
                Command::PrepareGraphProjection { graph, scope }
            }
        },
    )
}

#[cfg(test)]
mod tests;
