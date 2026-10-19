//! Exact physical typing and finite wire eligibility are separate contracts.
use super::{QuantityValueRefusal, QuantityValueView};
use crate::ExactNumericWork;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityValueProfile {
    Decimal,
    Exact,
    /// Explicit optional symbolic-coordinate grant; not a default Host profile.
    Mathematical,
}
impl QuantityValueProfile {
    pub const fn maximum_encoded_bytes(self) -> usize {
        match self {
            Self::Decimal => crate::QUANTITY_ENCODED_LEN,
            Self::Exact => super::QUANTITY_VALUE_MAX_ENCODED_BYTES,
            Self::Mathematical => super::MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES,
        }
    }
    /// Conservative producer-to-consumer eligibility, before inspecting values.
    /// An explicit numerical projection is required to narrow an Exact producer.
    pub const fn accepts(self, producer: Self) -> bool {
        matches!(
            (self, producer),
            (
                Self::Mathematical,
                Self::Decimal | Self::Exact | Self::Mathematical
            ) | (Self::Exact, Self::Decimal | Self::Exact)
                | (Self::Decimal, Self::Decimal)
        )
    }
    pub const fn admits_version(self, version: u8) -> bool {
        version == 1
            || (matches!(self, Self::Exact | Self::Mathematical) && version == 2)
            || (matches!(self, Self::Mathematical) && version == 3)
    }
    pub fn decode<'a>(
        self,
        identity: &str,
        encoded: &'a [u8],
        work: &mut ExactNumericWork,
    ) -> Result<QuantityValueView<'a>, QuantityValueContractRefusal> {
        if identity != crate::QUANTITY_INFO_ID
            && crate::parse_quantity_role_info_id(identity).is_none()
            && crate::quantity_info_dimension(identity).is_none()
        {
            return Err(QuantityValueContractRefusal::NotQuantityKind);
        }
        // Reject an unsupported wire form without invoking its numerical codec.
        if let Some(version) = encoded.first() {
            if !self.admits_version(*version) {
                return Err(QuantityValueContractRefusal::UnsupportedRepresentation {
                    profile: self,
                    version: *version,
                });
            }
        }
        if encoded.len() > self.maximum_encoded_bytes() {
            return Err(QuantityValueContractRefusal::ExtentExceeded);
        }
        let value = if encoded.first() == Some(&3) {
            QuantityValueView::decode_mathematical(encoded, work)
        } else {
            QuantityValueView::decode(encoded, work)
        }
        .map_err(QuantityValueContractRefusal::Encoding)?;
        value.validate_kind(identity)?;
        Ok(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityValueContractRefusal {
    Encoding(QuantityValueRefusal),
    NotQuantityKind,
    WrongFamily,
    WrongRole,
    WrongDimension {
        comparison: [u8; 32],
    },
    ExtentExceeded,
    UnsupportedRepresentation {
        profile: QuantityValueProfile,
        version: u8,
    },
}
impl QuantityValueView<'_> {
    /// A wire representation does not weaken the declared physical Type.
    pub fn validate_kind(self, identity: &str) -> Result<(), QuantityValueContractRefusal> {
        validate_quantity_kind(self.unit(), self.role(), identity)
    }
}

pub(super) fn validate_quantity_kind(
    unit: crate::Unit,
    actual_role: crate::QuantityRole,
    identity: &str,
) -> Result<(), QuantityValueContractRefusal> {
    use QuantityValueContractRefusal as R;
    if let Some((family, role)) = crate::parse_quantity_role_info_id(identity) {
        if unit.family().identity() != family {
            return Err(R::WrongFamily);
        }
        if actual_role != role {
            return Err(R::WrongRole);
        }
        return Ok(());
    }
    if identity == crate::QUANTITY_INFO_ID {
        return Ok(());
    }
    let expected = crate::quantity_info_dimension(identity).ok_or(R::NotQuantityKind)?;
    let actual = unit.dimension();
    if actual != expected {
        let mut descriptors = [0; 2 * crate::DIMENSION_DEFINITION_ENCODED_LEN];
        descriptors[..crate::DIMENSION_DEFINITION_ENCODED_LEN].copy_from_slice(&expected.encode());
        descriptors[crate::DIMENSION_DEFINITION_ENCODED_LEN..].copy_from_slice(&actual.encode());
        return Err(R::WrongDimension {
            comparison: crate::semantic_digest("physical/dimension-mismatch@1", &descriptors),
        });
    }
    // Preserve the legacy named-dimension contract. Family/role leaves use
    // the exact intrinsic identity branch above rather than this fallback.
    Ok(())
}
