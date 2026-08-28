use alloc::string::{String, ToString};

use conduit_core::{BootId, HostId, ResourcePoolId};
use serde::{Deserialize, Serialize};

use crate::{
    MAXIMUM_CREDENTIAL_BYTES, MAXIMUM_JOIN_INPUT_BYTES, MAXIMUM_JOIN_OUTPUT_BYTES,
    MAXIMUM_SSID_BYTES, NETWORK_JOIN_WIRE_VERSION,
};

const NETWORK_JOIN_WIRE_MAGIC: [u8; 4] = *b"CNJ1";
const NETWORK_JOIN_WIRE_HEADER_BYTES: usize = 7;
const NETWORK_ATTACHMENT_WIRE_MAGIC: [u8; 4] = *b"CNA1";
const NETWORK_ATTACHMENT_WIRE_VERSION: u8 = 1;
const NETWORK_ATTACHMENT_WIRE_HEADER_BYTES: usize = 21;
const MAXIMUM_ATTACHMENT_COMPONENT_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NetworkAttachmentId(String);

impl From<&str> for NetworkAttachmentId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl NetworkAttachmentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Boot-scoped runtime truth produced after successful Base execution.
/// It deliberately contains no SSID, credential, address, socket, or Line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkAttachment {
    pub attachment_id: NetworkAttachmentId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub interface_pool_id: ResourcePoolId,
    pub generation: u64,
}

/// Volatile Base input. Secret bytes intentionally implement neither
/// serialization nor display/debug formatting.
pub struct NetworkJoinRequest<'a> {
    pub ssid: &'a [u8],
    pub credential: &'a [u8],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NetworkAttachmentInfo<'a> {
    pub attachment_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub interface_pool_id: &'a str,
    pub generation: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NetworkJoinError {
    MalformedRequest,
    CredentialTooLarge,
    StaleHostBoot,
    Unsupported,
    MissingResource,
    ResourceMismatch,
    MissingAuthority,
    StaleAuthority,
    AuthorityMismatch,
    InvalidAttachment,
    OutputTooSmall,
}

pub fn encode_network_attachment(
    attachment: NetworkAttachmentInfo<'_>,
    output: &mut [u8],
) -> Result<usize, NetworkJoinError> {
    let fields = [
        attachment.attachment_id.as_bytes(),
        attachment.host_id.as_bytes(),
        attachment.boot_id.as_bytes(),
        attachment.interface_pool_id.as_bytes(),
    ];
    if attachment.generation == 0
        || fields
            .iter()
            .any(|field| field.is_empty() || field.len() > MAXIMUM_ATTACHMENT_COMPONENT_BYTES)
    {
        return Err(NetworkJoinError::InvalidAttachment);
    }
    let encoded_len = fields
        .iter()
        .try_fold(NETWORK_ATTACHMENT_WIRE_HEADER_BYTES, |length, field| {
            length.checked_add(field.len())
        })
        .ok_or(NetworkJoinError::InvalidAttachment)?;
    if encoded_len > MAXIMUM_JOIN_OUTPUT_BYTES as usize || output.len() < encoded_len {
        return Err(NetworkJoinError::OutputTooSmall);
    }
    output[..4].copy_from_slice(&NETWORK_ATTACHMENT_WIRE_MAGIC);
    output[4] = NETWORK_ATTACHMENT_WIRE_VERSION;
    for (index, field) in fields.iter().enumerate() {
        let start = 5 + index * 2;
        output[start..start + 2].copy_from_slice(&(field.len() as u16).to_le_bytes());
    }
    output[13..21].copy_from_slice(&attachment.generation.to_le_bytes());
    let mut cursor = NETWORK_ATTACHMENT_WIRE_HEADER_BYTES;
    for field in fields {
        output[cursor..cursor + field.len()].copy_from_slice(field);
        cursor += field.len();
    }
    Ok(encoded_len)
}

pub fn decode_network_attachment(
    encoded: &[u8],
) -> Result<NetworkAttachmentInfo<'_>, NetworkJoinError> {
    if encoded.len() < NETWORK_ATTACHMENT_WIRE_HEADER_BYTES
        || encoded.len() > MAXIMUM_JOIN_OUTPUT_BYTES as usize
        || encoded[..4] != NETWORK_ATTACHMENT_WIRE_MAGIC
        || encoded[4] != NETWORK_ATTACHMENT_WIRE_VERSION
    {
        return Err(NetworkJoinError::InvalidAttachment);
    }
    let mut lengths = [0_usize; 4];
    for (index, length) in lengths.iter_mut().enumerate() {
        let start = 5 + index * 2;
        *length = usize::from(u16::from_le_bytes([encoded[start], encoded[start + 1]]));
        if *length == 0 || *length > MAXIMUM_ATTACHMENT_COMPONENT_BYTES {
            return Err(NetworkJoinError::InvalidAttachment);
        }
    }
    let generation = u64::from_le_bytes(
        encoded[13..21]
            .try_into()
            .map_err(|_| NetworkJoinError::InvalidAttachment)?,
    );
    let mut cursor = NETWORK_ATTACHMENT_WIRE_HEADER_BYTES;
    let mut fields = [""; 4];
    for (index, length) in lengths.into_iter().enumerate() {
        let end = cursor
            .checked_add(length)
            .ok_or(NetworkJoinError::InvalidAttachment)?;
        let bytes = encoded
            .get(cursor..end)
            .ok_or(NetworkJoinError::InvalidAttachment)?;
        fields[index] =
            core::str::from_utf8(bytes).map_err(|_| NetworkJoinError::InvalidAttachment)?;
        cursor = end;
    }
    if cursor != encoded.len() || generation == 0 {
        return Err(NetworkJoinError::InvalidAttachment);
    }
    Ok(NetworkAttachmentInfo {
        attachment_id: fields[0],
        host_id: fields[1],
        boot_id: fields[2],
        interface_pool_id: fields[3],
        generation,
    })
}

/// Encodes the volatile runtime Info carried by the planned Cord. The format is
/// deliberately not a serde schema: callers must provide the admitted finite
/// buffer, and the secret-bearing value has no Debug/Display implementation.
pub fn encode_network_join_request(
    request: NetworkJoinRequest<'_>,
    output: &mut [u8],
) -> Result<usize, NetworkJoinError> {
    validate_join_request(&request)?;
    let encoded_len = NETWORK_JOIN_WIRE_HEADER_BYTES
        .checked_add(request.ssid.len())
        .and_then(|len| len.checked_add(request.credential.len()))
        .ok_or(NetworkJoinError::CredentialTooLarge)?;
    if encoded_len > MAXIMUM_JOIN_INPUT_BYTES as usize || output.len() < encoded_len {
        return Err(NetworkJoinError::OutputTooSmall);
    }
    output[..4].copy_from_slice(&NETWORK_JOIN_WIRE_MAGIC);
    output[4] = NETWORK_JOIN_WIRE_VERSION;
    output[5] = request.ssid.len() as u8;
    output[6] = request.credential.len() as u8;
    let ssid_end = NETWORK_JOIN_WIRE_HEADER_BYTES + request.ssid.len();
    output[NETWORK_JOIN_WIRE_HEADER_BYTES..ssid_end].copy_from_slice(request.ssid);
    output[ssid_end..encoded_len].copy_from_slice(request.credential);
    Ok(encoded_len)
}

/// Borrows the secret-bearing fields directly from the admitted frame so the
/// Pico Base does not allocate, clone, retain, or render credentials.
pub fn decode_network_join_request(
    encoded: &[u8],
) -> Result<NetworkJoinRequest<'_>, NetworkJoinError> {
    if encoded.len() < NETWORK_JOIN_WIRE_HEADER_BYTES
        || encoded.len() > MAXIMUM_JOIN_INPUT_BYTES as usize
        || encoded[..4] != NETWORK_JOIN_WIRE_MAGIC
        || encoded[4] != NETWORK_JOIN_WIRE_VERSION
    {
        return Err(NetworkJoinError::MalformedRequest);
    }
    let ssid_len = usize::from(encoded[5]);
    let credential_len = usize::from(encoded[6]);
    let ssid_end = NETWORK_JOIN_WIRE_HEADER_BYTES
        .checked_add(ssid_len)
        .ok_or(NetworkJoinError::MalformedRequest)?;
    let credential_end = ssid_end
        .checked_add(credential_len)
        .ok_or(NetworkJoinError::MalformedRequest)?;
    if credential_end != encoded.len() {
        return Err(NetworkJoinError::MalformedRequest);
    }
    let request = NetworkJoinRequest {
        ssid: &encoded[NETWORK_JOIN_WIRE_HEADER_BYTES..ssid_end],
        credential: &encoded[ssid_end..credential_end],
    };
    validate_join_request(&request)?;
    Ok(request)
}

pub(crate) fn validate_join_request(
    request: &NetworkJoinRequest<'_>,
) -> Result<(), NetworkJoinError> {
    if request.ssid.is_empty()
        || request.ssid.len() > MAXIMUM_SSID_BYTES
        || core::str::from_utf8(request.ssid).is_err()
    {
        return Err(NetworkJoinError::MalformedRequest);
    }
    if request.credential.is_empty() || request.credential.len() > MAXIMUM_CREDENTIAL_BYTES {
        return Err(NetworkJoinError::CredentialTooLarge);
    }
    Ok(())
}
