//! Provider-neutral, exact, bounded n-dimensional numeric information.

use alloc::{string::String, vec::Vec};
use conduit_core::{semantic_digest, BoundedResourceRef, QuantityUnit};

pub const TENSOR_INFO_ID: &str = "data/tensor@1";
pub const TENSOR_ENCODING_VERSION: u8 = 1;
pub const MAXIMUM_TENSOR_RANK: usize = 8;
pub const MAXIMUM_TENSOR_AXIS_IDENTITY_BYTES: usize = 64;
pub const MAXIMUM_INLINE_TENSOR_BYTES: usize = 64 * 1024;
pub const MAXIMUM_TENSOR_BYTES: u64 = 1_u64 << 50;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TensorElement {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl TensorElement {
    pub const fn byte_width(self) -> u64 {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    pub const fn semantic_id(self) -> &'static str {
        match self {
            Self::I8 => "number/i8",
            Self::U8 => "number/u8",
            Self::I16 => "number/i16-le",
            Self::U16 => "number/u16-le",
            Self::I32 => "number/i32-le",
            Self::U32 => "number/u32-le",
            Self::I64 => "number/i64-le",
            Self::U64 => "number/u64-le",
            Self::F32 => "number/ieee754-f32-le",
            Self::F64 => "number/ieee754-f64-le",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorAxisRole {
    Batch,
    Time,
    Feature,
    Sensor,
    SpatialCoordinate,
    Frequency,
    Channel,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorAxis {
    pub role: TensorAxisRole,
    /// Optional finite domain name, such as `tongue-sensor` or `latent-feature`.
    pub identity: Option<String>,
    pub unit: Option<QuantityUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorBacking {
    Inline(Vec<u8>),
    Resource(BoundedResourceRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorValue {
    pub element: TensorElement,
    pub dimensions: Vec<u64>,
    pub axes: Vec<TensorAxis>,
    /// Identity of the exact canonical element bytes, independent of carrier.
    pub content_digest: [u8; 32],
    pub backing: TensorBacking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorSummary {
    pub element: TensorElement,
    pub dimensions: Vec<u64>,
    pub axes: Vec<TensorAxis>,
    pub elements: u64,
    pub bytes: u64,
    pub resource_identity: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorRefusal {
    RankOutOfBounds,
    ZeroDimension,
    AxisCountMismatch,
    AxisIdentityInvalid,
    ShapeOverflow,
    ByteBoundExceeded,
    InlinePayloadTooLarge,
    PayloadLengthMismatch,
    ResourceProfileMismatch,
    ResourceExtentMismatch,
    ContentIdentityMismatch,
    InvalidResource,
    UnsupportedEncodingVersion,
    UnsupportedElement,
    UnsupportedAxisRole,
    UnsupportedUnit,
    MalformedEncoding,
}

impl TensorValue {
    pub fn validate(&self) -> Result<(), TensorRefusal> {
        let bytes = self.byte_count()?;
        if self.axes.len() != self.dimensions.len() {
            return Err(TensorRefusal::AxisCountMismatch);
        }
        for axis in &self.axes {
            validate_axis_identity(axis.identity.as_deref())?;
            if let TensorAxisRole::Other(role) = &axis.role {
                validate_axis_identity(Some(role))?;
            }
        }
        match &self.backing {
            TensorBacking::Inline(payload) => {
                if payload.len() > MAXIMUM_INLINE_TENSOR_BYTES {
                    return Err(TensorRefusal::InlinePayloadTooLarge);
                }
                if u64::try_from(payload.len()).ok() != Some(bytes) {
                    return Err(TensorRefusal::PayloadLengthMismatch);
                }
                if tensor_content_digest(payload) != self.content_digest {
                    return Err(TensorRefusal::ContentIdentityMismatch);
                }
            }
            TensorBacking::Resource(reference) => {
                reference
                    .validate()
                    .map_err(|_| TensorRefusal::InvalidResource)?;
                if reference.content_profile.as_str() != self.resource_profile() {
                    return Err(TensorRefusal::ResourceProfileMismatch);
                }
                if reference.extent.bytes != bytes
                    || reference.extent.items != Some(self.element_count()?)
                {
                    return Err(TensorRefusal::ResourceExtentMismatch);
                }
                if reference.identity.digest() != self.content_digest {
                    return Err(TensorRefusal::ContentIdentityMismatch);
                }
            }
        }
        Ok(())
    }

    pub fn element_count(&self) -> Result<u64, TensorRefusal> {
        if self.dimensions.is_empty() || self.dimensions.len() > MAXIMUM_TENSOR_RANK {
            return Err(TensorRefusal::RankOutOfBounds);
        }
        self.dimensions.iter().try_fold(1_u64, |count, dimension| {
            if *dimension == 0 {
                return Err(TensorRefusal::ZeroDimension);
            }
            count
                .checked_mul(*dimension)
                .ok_or(TensorRefusal::ShapeOverflow)
        })
    }

    pub fn byte_count(&self) -> Result<u64, TensorRefusal> {
        let bytes = self
            .element_count()?
            .checked_mul(self.element.byte_width())
            .ok_or(TensorRefusal::ShapeOverflow)?;
        if bytes > MAXIMUM_TENSOR_BYTES {
            Err(TensorRefusal::ByteBoundExceeded)
        } else {
            Ok(bytes)
        }
    }

    pub fn resource_profile(&self) -> &'static str {
        match self.element {
            TensorElement::I8 => "tensor/elements-i8@1",
            TensorElement::U8 => "tensor/elements-u8@1",
            TensorElement::I16 => "tensor/elements-i16-le@1",
            TensorElement::U16 => "tensor/elements-u16-le@1",
            TensorElement::I32 => "tensor/elements-i32-le@1",
            TensorElement::U32 => "tensor/elements-u32-le@1",
            TensorElement::I64 => "tensor/elements-i64-le@1",
            TensorElement::U64 => "tensor/elements-u64-le@1",
            TensorElement::F32 => "tensor/elements-ieee754-f32-le@1",
            TensorElement::F64 => "tensor/elements-ieee754-f64-le@1",
        }
    }

    pub fn summary(&self) -> Result<TensorSummary, TensorRefusal> {
        self.validate()?;
        Ok(TensorSummary {
            element: self.element,
            dimensions: self.dimensions.clone(),
            axes: self.axes.clone(),
            elements: self.element_count()?,
            bytes: self.byte_count()?,
            resource_identity: match &self.backing {
                TensorBacking::Inline(_) => None,
                TensorBacking::Resource(reference) => Some(reference.identity.digest()),
            },
        })
    }
}

pub fn tensor_content_digest(payload: &[u8]) -> [u8; 32] {
    semantic_digest("data/tensor-content@1", payload)
}

pub(crate) fn validate_axis_identity(identity: Option<&str>) -> Result<(), TensorRefusal> {
    if identity
        .is_some_and(|value| value.is_empty() || value.len() > MAXIMUM_TENSOR_AXIS_IDENTITY_BYTES)
    {
        Err(TensorRefusal::AxisIdentityInvalid)
    } else {
        Ok(())
    }
}
