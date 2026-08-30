//! Bounded, allocation-free validation for one Host-assigned Plan projection.

#![no_std]

mod execution;
mod sha256;
mod single_source;

pub use execution::*;
pub use single_source::*;

pub const ASSIGNED_PLAN_SCHEMA: u16 = 2;
pub const ASSIGNED_PLAN_HEADER_BYTES: usize = 124;
pub const TINY_HOST_TOTAL_BYTES: u16 = 2_560;
const MAGIC: &[u8; 8] = b"CNDAP001";
pub const ASSIGNED_PLAN_COUNT_KINDS: usize = 12;

pub const ASSIGNED_NODE: u8 = 1;
pub const ASSIGNED_PORT: u8 = 2;
pub const ASSIGNED_CONFIGURATION: u8 = 3;
pub const ASSIGNED_CORD: u8 = 4;
pub const ASSIGNED_ROUTE: u8 = 5;
pub const ASSIGNED_ROUTE_TARGET: u8 = 6;
pub const ASSIGNED_HOST_OPERATION: u8 = 7;
pub const ASSIGNED_RESOURCE: u8 = 8;
pub const ASSIGNED_SIGN: u8 = 9;
pub const ASSIGNED_REMOTE_ENDPOINT: u8 = 10;
pub const ASSIGNED_STARTUP: u8 = 11;
pub const ASSIGNED_TERMINAL: u8 = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedIdentity(pub [u8; 16]);

impl AssignedIdentity {
    pub fn from_text(value: &str) -> Self {
        let digest = sha256::digest(value.as_bytes());
        let mut result = [0; 16];
        result.copy_from_slice(&digest[..16]);
        Self(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedRemoteBinding {
    pub line: AssignedIdentity,
    pub local_host: AssignedIdentity,
    pub local_boot: AssignedIdentity,
    pub peer_host: AssignedIdentity,
    pub peer_boot: AssignedIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedPlanMaxima {
    pub encoded_bytes: u16,
    pub runtime_state_bytes: u16,
    pub counts: [u8; ASSIGNED_PLAN_COUNT_KINDS],
}

impl AssignedPlanMaxima {
    /// Exact storage ceiling for the generic one-source tiny-Host profile.
    pub const SINGLE_SOURCE: Self = Self {
        encoded_bytes: 544,
        runtime_state_bytes: 192,
        counts: [1, 1, 0, 0, 0, 0, 1, 3, 4, 0, 1, 2],
    };

    pub const TINY_HOST: Self = Self {
        encoded_bytes: 1_536,
        runtime_state_bytes: 1_024,
        counts: [8, 16, 8, 8, 8, 8, 8, 8, 16, 2, 8, 8],
    };

    pub const fn total_bytes(self) -> u16 {
        self.encoded_bytes + self.runtime_state_bytes
    }
}

#[derive(Clone, Copy)]
pub struct AssignedPlanRequirements<'a> {
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
    pub operations: &'a [AssignedIdentity],
    pub resources: &'a [u16],
    pub remote_bindings: &'a [AssignedRemoteBinding],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssignedPlanView {
    pub plan: AssignedIdentity,
    pub fragment: AssignedIdentity,
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
    pub encoded_bytes: u16,
    pub runtime_state_bytes: u16,
    pub counts: [u8; ASSIGNED_PLAN_COUNT_KINDS],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedPlanRefusal {
    WrongLength,
    WrongMagic,
    WrongSchema,
    WrongHost,
    WrongBoot,
    EncodedCapacityExceeded,
    RuntimeCapacityExceeded,
    CountCapacityExceeded(u8),
    DigestMismatch,
    MalformedRecord,
    UnknownRecord(u8),
    UnknownOperation,
    MissingOperation,
    UnknownResource,
    MissingResource,
    StaleRemoteEndpoint,
    MissingRemoteEndpoint,
    ExtraRecords,
}

pub fn decode_assigned_plan(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    requirements: AssignedPlanRequirements<'_>,
) -> Result<AssignedPlanView, AssignedPlanRefusal> {
    if bytes.len() < ASSIGNED_PLAN_HEADER_BYTES {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if &bytes[..8] != MAGIC {
        return Err(AssignedPlanRefusal::WrongMagic);
    }
    if read_u16(bytes, 8)? != ASSIGNED_PLAN_SCHEMA {
        return Err(AssignedPlanRefusal::WrongSchema);
    }
    let encoded_bytes = read_u16(bytes, 10)?;
    if usize::from(encoded_bytes) != bytes.len() {
        return Err(AssignedPlanRefusal::WrongLength);
    }
    if encoded_bytes > maxima.encoded_bytes {
        return Err(AssignedPlanRefusal::EncodedCapacityExceeded);
    }
    let runtime_state_bytes = read_u16(bytes, 12)?;
    if runtime_state_bytes > maxima.runtime_state_bytes {
        return Err(AssignedPlanRefusal::RuntimeCapacityExceeded);
    }
    let plan = read_identity(bytes, 16)?;
    let fragment = read_identity(bytes, 32)?;
    let host = read_identity(bytes, 48)?;
    let boot = read_identity(bytes, 64)?;
    if host != requirements.host {
        return Err(AssignedPlanRefusal::WrongHost);
    }
    if boot != requirements.boot {
        return Err(AssignedPlanRefusal::WrongBoot);
    }
    let mut counts = [0; ASSIGNED_PLAN_COUNT_KINDS];
    counts.copy_from_slice(&bytes[80..92]);
    for (index, (actual, maximum)) in counts.iter().zip(maxima.counts).enumerate() {
        if *actual > maximum {
            return Err(AssignedPlanRefusal::CountCapacityExceeded(index as u8 + 1));
        }
    }
    let expected_digest = &bytes[92..124];
    let actual_digest = sha256::digest(&bytes[124..]);
    if actual_digest != expected_digest {
        return Err(AssignedPlanRefusal::DigestMismatch);
    }

    let mut seen = [0_u8; ASSIGNED_PLAN_COUNT_KINDS];
    let mut operations = [false; 32];
    let mut resources = [false; 32];
    let mut remotes = [false; 16];
    if requirements.operations.len() > operations.len()
        || requirements.resources.len() > resources.len()
        || requirements.remote_bindings.len() > remotes.len()
    {
        return Err(AssignedPlanRefusal::MalformedRecord);
    }
    let mut cursor = ASSIGNED_PLAN_HEADER_BYTES;
    while cursor < bytes.len() {
        let tag = *bytes
            .get(cursor)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?;
        let length = usize::from(read_u16(bytes, cursor + 1)?);
        let start = cursor
            .checked_add(3)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?;
        let end = start
            .checked_add(length)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?;
        let payload = bytes
            .get(start..end)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?;
        if tag == 0 || usize::from(tag) > ASSIGNED_PLAN_COUNT_KINDS {
            return Err(AssignedPlanRefusal::UnknownRecord(tag));
        }
        let valid_length = match tag {
            ASSIGNED_NODE => length == 52,
            ASSIGNED_PORT => length == 37,
            ASSIGNED_CONFIGURATION => length == 27,
            ASSIGNED_CORD => length == 36,
            ASSIGNED_ROUTE => length == 8,
            ASSIGNED_ROUTE_TARGET => length == 6 || length == 7,
            ASSIGNED_HOST_OPERATION => length == 46,
            ASSIGNED_RESOURCE => length == 8,
            ASSIGNED_SIGN => length == 37,
            ASSIGNED_REMOTE_ENDPOINT => length == 250,
            ASSIGNED_STARTUP => length == 3 || length == 5,
            ASSIGNED_TERMINAL => length == 32,
            _ => false,
        };
        if !valid_length {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        let valid_discriminants = match tag {
            ASSIGNED_PORT => payload[4] <= 1,
            ASSIGNED_CONFIGURATION => payload[18] <= 2,
            ASSIGNED_CORD => payload[18] <= 1 && payload[23] <= 1,
            ASSIGNED_ROUTE_TARGET => length == 6 || payload[2] == 1,
            ASSIGNED_SIGN => payload[34] <= 2,
            ASSIGNED_REMOTE_ENDPOINT => {
                payload[228] <= 1
                    && payload[229] <= 4
                    && payload[230] <= 2
                    && payload[231] <= 2
                    && payload[232] <= 1
                    && payload[233] <= 1
                    && payload[234] <= 1
                    && payload[235] <= 3
            }
            ASSIGNED_STARTUP => {
                (payload[0] == 0 && length == 5) || (payload[0] == 1 && length == 3)
            }
            _ => true,
        };
        if !valid_discriminants {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        let count = &mut seen[usize::from(tag - 1)];
        *count = count
            .checked_add(1)
            .ok_or(AssignedPlanRefusal::ExtraRecords)?;
        match tag {
            ASSIGNED_HOST_OPERATION => {
                let identity = read_identity(payload, 4)?;
                let index = requirements
                    .operations
                    .iter()
                    .enumerate()
                    .find(|(index, required)| !operations[*index] && **required == identity)
                    .map(|(index, _)| index)
                    .ok_or(AssignedPlanRefusal::UnknownOperation)?;
                operations[index] = true;
            }
            ASSIGNED_RESOURCE => {
                let resource = read_u16(payload, 2)?;
                let index = requirements
                    .resources
                    .iter()
                    .enumerate()
                    .find(|(index, required)| !resources[*index] && **required == resource)
                    .map(|(index, _)| index)
                    .ok_or(AssignedPlanRefusal::UnknownResource)?;
                resources[index] = true;
            }
            ASSIGNED_REMOTE_ENDPOINT => {
                let binding = AssignedRemoteBinding {
                    line: read_identity(payload, 0)?,
                    local_host: read_identity(payload, 16)?,
                    local_boot: read_identity(payload, 32)?,
                    peer_host: read_identity(payload, 48)?,
                    peer_boot: read_identity(payload, 64)?,
                };
                let index = requirements
                    .remote_bindings
                    .iter()
                    .enumerate()
                    .find(|(index, required)| !remotes[*index] && **required == binding)
                    .map(|(index, _)| index)
                    .ok_or(AssignedPlanRefusal::StaleRemoteEndpoint)?;
                remotes[index] = true;
            }
            _ => {}
        }
        cursor = end;
    }
    if seen != counts {
        return Err(AssignedPlanRefusal::ExtraRecords);
    }
    if operations[..requirements.operations.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(AssignedPlanRefusal::MissingOperation);
    }
    if resources[..requirements.resources.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(AssignedPlanRefusal::MissingResource);
    }
    if remotes[..requirements.remote_bindings.len()]
        .iter()
        .any(|seen| !seen)
    {
        return Err(AssignedPlanRefusal::MissingRemoteEndpoint);
    }
    Ok(AssignedPlanView {
        plan,
        fragment,
        host,
        boot,
        encoded_bytes,
        runtime_state_bytes,
        counts,
    })
}

pub fn assigned_plan_payload_digest(payload: &[u8]) -> [u8; 32] {
    sha256::digest(payload)
}

pub fn assigned_plan_magic() -> [u8; 8] {
    *MAGIC
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<AssignedIdentity, AssignedPlanRefusal> {
    let mut result = [0; 16];
    result.copy_from_slice(
        bytes
            .get(offset..offset + 16)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?,
    );
    Ok(AssignedIdentity(result))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, AssignedPlanRefusal> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(AssignedPlanRefusal::MalformedRecord)?
            .try_into()
            .map_err(|_| AssignedPlanRefusal::MalformedRecord)?,
    ))
}
