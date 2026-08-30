//! Compact serialization of existing Play activation and terminal evidence.
//!
//! These messages do not define another lifecycle. They carry the exact
//! generic Plan, fragment, Host, Boot, ActivePlay, Port, and disposition
//! identities needed by a tiny Host that cannot deserialize hosted types.

use crate::{sha256, AssignedIdentity};

const ACTIVATION_MAGIC: &[u8; 8] = b"CNDAC001";
const RECEIPT_MAGIC: &[u8; 8] = b"CNDRE001";
const SCHEMA: u16 = 1;
pub const ASSIGNED_ACTIVATION_BYTES: usize = 124;
pub const ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES: usize = 131;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedActivation {
    pub plan: AssignedIdentity,
    pub fragment: AssignedIdentity,
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
    pub active_play: AssignedIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssignedTerminalDisposition {
    Completed = 0,
    Refused = 1,
    Failed = 2,
    Cancelled = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedExecutionReceipt<'a> {
    pub activation: AssignedActivation,
    pub output_port: u16,
    pub disposition: AssignedTerminalDisposition,
    pub detail: u16,
    pub value: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedExecutionRefusal {
    WrongLength,
    WrongMagic,
    WrongSchema,
    DigestMismatch,
    OutputTooSmall,
    InvalidDisposition,
}

pub fn encode_assigned_activation(
    activation: AssignedActivation,
) -> [u8; ASSIGNED_ACTIVATION_BYTES] {
    let mut output = [0_u8; ASSIGNED_ACTIVATION_BYTES];
    output[..8].copy_from_slice(ACTIVATION_MAGIC);
    output[8..10].copy_from_slice(&SCHEMA.to_le_bytes());
    output[10..12].copy_from_slice(&(ASSIGNED_ACTIVATION_BYTES as u16).to_le_bytes());
    identities_to(&mut output[12..92], activation);
    let digest = sha256::digest(&output[..92]);
    output[92..].copy_from_slice(&digest);
    output
}

#[inline(never)]
pub fn decode_assigned_activation(
    bytes: &[u8],
) -> Result<AssignedActivation, AssignedExecutionRefusal> {
    if bytes.len() != ASSIGNED_ACTIVATION_BYTES {
        return Err(AssignedExecutionRefusal::WrongLength);
    }
    if &bytes[..8] != ACTIVATION_MAGIC {
        return Err(AssignedExecutionRefusal::WrongMagic);
    }
    if u16_at(bytes, 8)? != SCHEMA || usize::from(u16_at(bytes, 10)?) != bytes.len() {
        return Err(AssignedExecutionRefusal::WrongSchema);
    }
    if sha256::digest(&bytes[..92]) != bytes[92..] {
        return Err(AssignedExecutionRefusal::DigestMismatch);
    }
    Ok(identities_from(&bytes[12..92])?)
}

#[inline(never)]
pub fn encode_assigned_execution_receipt(
    receipt: AssignedExecutionReceipt<'_>,
    output: &mut [u8],
) -> Result<usize, AssignedExecutionRefusal> {
    let total = ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES
        .checked_add(receipt.value.len())
        .ok_or(AssignedExecutionRefusal::WrongLength)?;
    let total_u16 = u16::try_from(total).map_err(|_| AssignedExecutionRefusal::WrongLength)?;
    let target = output
        .get_mut(..total)
        .ok_or(AssignedExecutionRefusal::OutputTooSmall)?;
    target.fill(0);
    target[..8].copy_from_slice(RECEIPT_MAGIC);
    target[8..10].copy_from_slice(&SCHEMA.to_le_bytes());
    target[10..12].copy_from_slice(&total_u16.to_le_bytes());
    identities_to(&mut target[12..92], receipt.activation);
    target[92..94].copy_from_slice(&receipt.output_port.to_le_bytes());
    target[94] = receipt.disposition as u8;
    target[95..97].copy_from_slice(&receipt.detail.to_le_bytes());
    target[97..99].copy_from_slice(&(receipt.value.len() as u16).to_le_bytes());
    let digest = sha256::digest(receipt.value);
    target[99..131].copy_from_slice(&digest);
    target[131..].copy_from_slice(receipt.value);
    Ok(total)
}

pub fn decode_assigned_execution_receipt(
    bytes: &[u8],
) -> Result<AssignedExecutionReceipt<'_>, AssignedExecutionRefusal> {
    if bytes.len() < ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES {
        return Err(AssignedExecutionRefusal::WrongLength);
    }
    if &bytes[..8] != RECEIPT_MAGIC {
        return Err(AssignedExecutionRefusal::WrongMagic);
    }
    if u16_at(bytes, 8)? != SCHEMA {
        return Err(AssignedExecutionRefusal::WrongSchema);
    }
    let total = usize::from(u16_at(bytes, 10)?);
    let value_len = usize::from(u16_at(bytes, 97)?);
    if total != bytes.len()
        || ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES.checked_add(value_len) != Some(total)
    {
        return Err(AssignedExecutionRefusal::WrongLength);
    }
    let value = &bytes[ASSIGNED_EXECUTION_RECEIPT_HEADER_BYTES..];
    if sha256::digest(value) != bytes[99..131] {
        return Err(AssignedExecutionRefusal::DigestMismatch);
    }
    let disposition = match bytes[94] {
        0 => AssignedTerminalDisposition::Completed,
        1 => AssignedTerminalDisposition::Refused,
        2 => AssignedTerminalDisposition::Failed,
        3 => AssignedTerminalDisposition::Cancelled,
        _ => return Err(AssignedExecutionRefusal::InvalidDisposition),
    };
    Ok(AssignedExecutionReceipt {
        activation: identities_from(&bytes[12..92])?,
        output_port: u16_at(bytes, 92)?,
        disposition,
        detail: u16_at(bytes, 95)?,
        value,
    })
}

fn identities_to(target: &mut [u8], value: AssignedActivation) {
    for (index, identity) in [
        value.plan,
        value.fragment,
        value.host,
        value.boot,
        value.active_play,
    ]
    .iter()
    .enumerate()
    {
        target[index * 16..(index + 1) * 16].copy_from_slice(&identity.0);
    }
}

fn identities_from(bytes: &[u8]) -> Result<AssignedActivation, AssignedExecutionRefusal> {
    let identity = |index: usize| {
        bytes
            .get(index * 16..(index + 1) * 16)
            .ok_or(AssignedExecutionRefusal::WrongLength)?
            .try_into()
            .map(AssignedIdentity)
            .map_err(|_| AssignedExecutionRefusal::WrongLength)
    };
    Ok(AssignedActivation {
        plan: identity(0)?,
        fragment: identity(1)?,
        host: identity(2)?,
        boot: identity(3)?,
        active_play: identity(4)?,
    })
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, AssignedExecutionRefusal> {
    bytes
        .get(offset..offset + 2)
        .ok_or(AssignedExecutionRefusal::WrongLength)?
        .try_into()
        .map(u16::from_le_bytes)
        .map_err(|_| AssignedExecutionRefusal::WrongLength)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activation() -> AssignedActivation {
        AssignedActivation {
            plan: AssignedIdentity([1; 16]),
            fragment: AssignedIdentity([2; 16]),
            host: AssignedIdentity([3; 16]),
            boot: AssignedIdentity([4; 16]),
            active_play: AssignedIdentity([5; 16]),
        }
    }

    #[test]
    fn activation_and_one_terminal_receipt_round_trip_and_bind_every_identity() {
        let encoded = encode_assigned_activation(activation());
        assert_eq!(decode_assigned_activation(&encoded), Ok(activation()));

        let mut bytes = [0_u8; 160];
        let length = encode_assigned_execution_receipt(
            AssignedExecutionReceipt {
                activation: activation(),
                output_port: 7,
                disposition: AssignedTerminalDisposition::Completed,
                detail: 0,
                value: &[0x06],
            },
            &mut bytes,
        )
        .unwrap();
        let decoded = decode_assigned_execution_receipt(&bytes[..length]).unwrap();
        assert_eq!(decoded.activation, activation());
        assert_eq!(decoded.output_port, 7);
        assert_eq!(decoded.value, [0x06]);

        bytes[length - 1] ^= 1;
        assert_eq!(
            decode_assigned_execution_receipt(&bytes[..length]),
            Err(AssignedExecutionRefusal::DigestMismatch)
        );
    }
}
