//! Exact bounded references to large typed content.
//!
//! A reference is portable semantic Info. It deliberately contains no path,
//! URL, socket, credential, Host-local handle, or ambient authority. Opening
//! the referenced content remains a separately admitted effect.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    semantic_digest, KindId, ResourceClassId, TemporalInstant, TemporalRelation,
    TemporalRelationError, TemporalScale,
};

pub const RESOURCE_REFERENCE_INFO_ID: &str = "value/resource-ref@1";
pub const RESOURCE_REFERENCE_ENCODING_VERSION: u8 = 1;
pub const RESOURCE_REFERENCE_DIGEST_BYTES: usize = 32;
pub const MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_REFERENCED_BYTES: u64 = 1_u64 << 50;
pub const MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES: usize = 512;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceSemanticIdentity([u8; RESOURCE_REFERENCE_DIGEST_BYTES]);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceVersionIdentity([u8; RESOURCE_REFERENCE_DIGEST_BYTES]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ResourceExtent {
    pub bytes: u64,
    pub items: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLifetime {
    pub version: ResourceVersionIdentity,
    pub expires_at: Option<TemporalInstant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedResourceRef {
    pub identity: ResourceSemanticIdentity,
    pub content_profile: KindId,
    pub access_class: ResourceClassId,
    pub extent: ResourceExtent,
    pub lifetime: ResourceLifetime,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceReferenceRefusal {
    ZeroSemanticIdentity,
    ZeroVersionIdentity,
    EmptyContentProfile,
    ContentProfileTooLarge,
    EmptyAccessClass,
    AccessClassTooLarge,
    ByteBoundExceeded,
    InvalidExpiry,
    IncomparableExpiry,
    EncodingTooLarge,
    MalformedEncoding,
    UnsupportedEncodingVersion,
}

impl ResourceSemanticIdentity {
    pub const fn from_digest(digest: [u8; RESOURCE_REFERENCE_DIGEST_BYTES]) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> [u8; RESOURCE_REFERENCE_DIGEST_BYTES] {
        self.0
    }

    fn validate(self) -> Result<(), ResourceReferenceRefusal> {
        if self.0 == [0; RESOURCE_REFERENCE_DIGEST_BYTES] {
            Err(ResourceReferenceRefusal::ZeroSemanticIdentity)
        } else {
            Ok(())
        }
    }
}

impl ResourceVersionIdentity {
    pub const fn from_digest(digest: [u8; RESOURCE_REFERENCE_DIGEST_BYTES]) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> [u8; RESOURCE_REFERENCE_DIGEST_BYTES] {
        self.0
    }

    fn validate(self) -> Result<(), ResourceReferenceRefusal> {
        if self.0 == [0; RESOURCE_REFERENCE_DIGEST_BYTES] {
            Err(ResourceReferenceRefusal::ZeroVersionIdentity)
        } else {
            Ok(())
        }
    }
}

impl BoundedResourceRef {
    pub fn validate(&self) -> Result<(), ResourceReferenceRefusal> {
        self.identity.validate()?;
        self.lifetime.version.validate()?;
        validate_identity(
            self.content_profile.as_str(),
            ResourceReferenceRefusal::EmptyContentProfile,
            ResourceReferenceRefusal::ContentProfileTooLarge,
        )?;
        validate_identity(
            self.access_class.as_str(),
            ResourceReferenceRefusal::EmptyAccessClass,
            ResourceReferenceRefusal::AccessClassTooLarge,
        )?;
        if self.extent.bytes > MAXIMUM_REFERENCED_BYTES {
            return Err(ResourceReferenceRefusal::ByteBoundExceeded);
        }
        if let Some(expires_at) = &self.lifetime.expires_at {
            expires_at
                .validate()
                .map_err(|_| ResourceReferenceRefusal::InvalidExpiry)?;
        }
        Ok(())
    }

    pub fn expiry_relation(
        &self,
        reference: &TemporalInstant,
    ) -> Result<Option<TemporalRelation>, ResourceReferenceRefusal> {
        self.validate()?;
        reference
            .validate()
            .map_err(|_| ResourceReferenceRefusal::InvalidExpiry)?;
        self.lifetime
            .expires_at
            .as_ref()
            .map(|expiry| {
                expiry
                    .relation_to(reference)
                    .map_err(map_expiry_relation_error)
            })
            .transpose()
    }

    pub fn encode(&self) -> Result<Vec<u8>, ResourceReferenceRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.push(RESOURCE_REFERENCE_ENCODING_VERSION);
        bytes.extend_from_slice(&self.identity.digest());
        bytes.extend_from_slice(&self.lifetime.version.digest());
        push_identity(&mut bytes, self.content_profile.as_str());
        push_identity(&mut bytes, self.access_class.as_str());
        bytes.extend_from_slice(&self.extent.bytes.to_le_bytes());
        push_optional_u64(&mut bytes, self.extent.items);
        match &self.lifetime.expires_at {
            None => bytes.push(0),
            Some(expiry) => {
                bytes.push(1);
                bytes.extend_from_slice(&expiry.ticks.to_le_bytes());
                bytes.push(encode_scale(expiry.scale));
                push_identity(&mut bytes, &expiry.clock_basis);
                bytes.extend_from_slice(&expiry.resolution_ticks.to_le_bytes());
                bytes.extend_from_slice(&expiry.uncertainty_ticks.to_le_bytes());
            }
        }
        if bytes.len() > MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES {
            return Err(ResourceReferenceRefusal::EncodingTooLarge);
        }
        Ok(bytes)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ResourceReferenceRefusal> {
        if encoded.len() > MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES {
            return Err(ResourceReferenceRefusal::EncodingTooLarge);
        }
        let mut cursor = Cursor::new(encoded);
        if cursor.u8()? != RESOURCE_REFERENCE_ENCODING_VERSION {
            return Err(ResourceReferenceRefusal::UnsupportedEncodingVersion);
        }
        let identity = ResourceSemanticIdentity::from_digest(cursor.digest()?);
        let version = ResourceVersionIdentity::from_digest(cursor.digest()?);
        let content_profile = KindId::from(cursor.identity()?);
        let access_class = ResourceClassId::from(cursor.identity()?);
        let extent = ResourceExtent {
            bytes: cursor.u64()?,
            items: cursor.optional_u64()?,
        };
        let expires_at = match cursor.u8()? {
            0 => None,
            1 => Some(TemporalInstant {
                ticks: cursor.u64()?,
                scale: decode_scale(cursor.u8()?)?,
                clock_basis: cursor.identity()?,
                resolution_ticks: cursor.u64()?,
                uncertainty_ticks: cursor.u64()?,
            }),
            _ => return Err(ResourceReferenceRefusal::MalformedEncoding),
        };
        if !cursor.finished() {
            return Err(ResourceReferenceRefusal::MalformedEncoding);
        }
        let value = Self {
            identity,
            content_profile,
            access_class,
            extent,
            lifetime: ResourceLifetime {
                version,
                expires_at,
            },
        };
        value.validate()?;
        Ok(value)
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], ResourceReferenceRefusal> {
        Ok(semantic_digest(RESOURCE_REFERENCE_INFO_ID, &self.encode()?))
    }
}

fn validate_identity(
    value: &str,
    empty: ResourceReferenceRefusal,
    too_large: ResourceReferenceRefusal,
) -> Result<(), ResourceReferenceRefusal> {
    if value.is_empty() {
        return Err(empty);
    }
    if value.len() > MAXIMUM_RESOURCE_REFERENCE_IDENTITY_BYTES {
        return Err(too_large);
    }
    Ok(())
}

fn push_identity(encoded: &mut Vec<u8>, value: &str) {
    encoded.extend_from_slice(&(value.len() as u16).to_le_bytes());
    encoded.extend_from_slice(value.as_bytes());
}

fn push_optional_u64(encoded: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => encoded.push(0),
        Some(value) => {
            encoded.push(1);
            encoded.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_scale(scale: TemporalScale) -> u8 {
    match scale {
        TemporalScale::Seconds => 0,
        TemporalScale::Milliseconds => 1,
        TemporalScale::Microseconds => 2,
        TemporalScale::Nanoseconds => 3,
    }
}

fn decode_scale(encoded: u8) -> Result<TemporalScale, ResourceReferenceRefusal> {
    match encoded {
        0 => Ok(TemporalScale::Seconds),
        1 => Ok(TemporalScale::Milliseconds),
        2 => Ok(TemporalScale::Microseconds),
        3 => Ok(TemporalScale::Nanoseconds),
        _ => Err(ResourceReferenceRefusal::MalformedEncoding),
    }
}

fn map_expiry_relation_error(error: TemporalRelationError) -> ResourceReferenceRefusal {
    match error {
        TemporalRelationError::Incomparable => ResourceReferenceRefusal::IncomparableExpiry,
        TemporalRelationError::InvalidInstant | TemporalRelationError::IntervalOverflow => {
            ResourceReferenceRefusal::InvalidExpiry
        }
    }
}

struct Cursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ResourceReferenceRefusal> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ResourceReferenceRefusal::MalformedEncoding)?;
        let value = self
            .encoded
            .get(self.position..end)
            .ok_or(ResourceReferenceRefusal::MalformedEncoding)?;
        self.position = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ResourceReferenceRefusal> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ResourceReferenceRefusal> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u64(&mut self) -> Result<u64, ResourceReferenceRefusal> {
        self.take(8)?
            .try_into()
            .map(u64::from_le_bytes)
            .map_err(|_| ResourceReferenceRefusal::MalformedEncoding)
    }

    fn digest(
        &mut self,
    ) -> Result<[u8; RESOURCE_REFERENCE_DIGEST_BYTES], ResourceReferenceRefusal> {
        self.take(RESOURCE_REFERENCE_DIGEST_BYTES)?
            .try_into()
            .map_err(|_| ResourceReferenceRefusal::MalformedEncoding)
    }

    fn identity(&mut self) -> Result<String, ResourceReferenceRefusal> {
        let length = usize::from(self.u16()?);
        let bytes = self.take(length)?;
        core::str::from_utf8(bytes)
            .map(String::from)
            .map_err(|_| ResourceReferenceRefusal::MalformedEncoding)
    }

    fn optional_u64(&mut self) -> Result<Option<u64>, ResourceReferenceRefusal> {
        match self.u8()? {
            0 => Ok(None),
            1 => self.u64().map(Some),
            _ => Err(ResourceReferenceRefusal::MalformedEncoding),
        }
    }

    fn finished(&self) -> bool {
        self.position == self.encoded.len()
    }
}
