//! Private publication transfer; the complete ordinary Plan stays typed.
//! Carry complete ordinary models through bounded CBOR, as the native Crèche
//! already does. Public JSON and canonical admission remain unchanged.
mod decoder;
mod preflight;
mod show;
use super::{Command, Refusal};
use conduit_body::BodyId;
use conduit_face::{Face, FaceContributionBasis};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8] = b"CAPP\x02";
// Existing finite expanded Plan-value ceiling, measured in this private carrier.
const MAX_PLAN_CBOR_BYTES: usize = 256 * 1024;

#[derive(Serialize)]
struct Borrowed<'a> {
    slot: u8,
    body: &'a BodyId,
    basis: &'a FaceContributionBasis,
    face: &'a Face,
    show: show::ShowRef<'a>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Owned {
    slot: u8,
    body: BodyId,
    basis: FaceContributionBasis,
    face: Face,
    show: show::ShowOwned,
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
            show: show::ShowRef::from(show),
        },
        &mut remaining,
    )
    .map_err(|_| Refusal::Capacity)?;
    let length = capacity - remaining.len();
    preflight(&output[..length])?;
    Ok(length)
}
pub(super) fn preflight(bytes: &[u8]) -> Result<(), Refusal> {
    preflight::validate(
        bytes.strip_prefix(MAGIC).ok_or(Refusal::Malformed)?,
        MAX_PLAN_CBOR_BYTES,
    )
}
/// Only the immutable bytes already checked by Workspace may reach this path.
pub(super) fn decode_preflighted(bytes: &[u8]) -> Result<Command, Refusal> {
    let value: Owned = decoder::decode(bytes.strip_prefix(MAGIC).ok_or(Refusal::Malformed)?)
        .map_err(|_| Refusal::Malformed)?;
    Ok(Command::PatchbayPublish {
        slot: value.slot,
        body: value.body,
        basis: value.basis,
        face: value.face,
        show: value.show.into(),
    })
}

#[cfg(test)]
mod tests;
