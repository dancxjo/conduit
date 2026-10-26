//! Private publication transfer; the complete ordinary Plan stays typed.
//! Binary serde selects MaskShow's existing full Plan representation rather
//! than the public JSON compressed-Plan document. Public JSON is unchanged.
mod preflight;
use super::{Command, Refusal};
use conduit_body::BodyId;
use conduit_face::{Face, FaceContributionBasis, MaskShow};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8] = b"CAPB\x01";

#[derive(Serialize)]
struct Borrowed<'a> {
    slot: u8,
    body: &'a BodyId,
    basis: &'a FaceContributionBasis,
    face: &'a Face,
    show: &'a MaskShow,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Owned {
    slot: u8,
    body: BodyId,
    basis: FaceContributionBasis,
    face: Face,
    show: MaskShow,
}

pub(super) fn matches(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
pub(super) fn encode(command: &Command, output: &mut [u8]) -> Result<usize, Refusal> {
    let Command::PatchbayPublish {
        slot,
        body,
        basis,
        face,
        show,
    } = command
    else {
        return Err(Refusal::Order);
    };
    output
        .get_mut(..MAGIC.len())
        .ok_or(Refusal::Capacity)?
        .copy_from_slice(MAGIC);
    let capacity = output.len();
    let mut remaining = &mut output[MAGIC.len()..];
    ciborium::ser::into_writer(
        &Borrowed {
            slot: *slot,
            body,
            basis,
            face,
            show,
        },
        &mut remaining,
    )
    .map_err(|_| Refusal::Capacity)?;
    Ok(capacity - remaining.len())
}
pub(super) fn preflight(bytes: &[u8]) -> Result<(), Refusal> {
    preflight::validate(bytes.strip_prefix(MAGIC).ok_or(Refusal::Malformed)?)
}
/// Only the immutable bytes already checked by Workspace may reach this path.
pub(super) fn decode_preflighted(bytes: &[u8]) -> Result<Command, Refusal> {
    let mut remaining = bytes.strip_prefix(MAGIC).ok_or(Refusal::Malformed)?;
    let value: Owned = ciborium::de::from_reader_with_recursion_limit(&mut remaining, 64)
        .map_err(|_| Refusal::Malformed)?;
    if !remaining.is_empty() {
        return Err(Refusal::Malformed);
    }
    Ok(Command::PatchbayPublish {
        slot: value.slot,
        body: value.body,
        basis: value.basis,
        face: value.face,
        show: value.show,
    })
}

#[cfg(test)]
mod tests;
