//! Canonical captured mathematical unit law (staged replacement).
use super::*;
use crate::{MathValue, MathValueRef, NumericRefusal, PortableNumericProfile};
use alloc::{boxed::Box, vec::Vec};
use core::hash::Hash;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrefixPolicy {
    decimal_exponents: u64,
    binary_exponents: u128,
    pub power: u8,
}
impl PrefixPolicy {
    pub const NONE: Self = Self {
        decimal_exponents: 0,
        binary_exponents: 0,
        power: 1,
    };
    pub fn new(
        decimal: &[i8],
        binary: &[u8],
        power: u8,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if !(1..=8).contains(&power) {
            return Err(PhysicalDefinitionRefusal::InvalidPrefix);
        }
        let mut value = Self {
            decimal_exponents: 0,
            binary_exponents: 0,
            power,
        };
        for &exponent in decimal {
            let bit = decimal_bit(exponent).ok_or(PhysicalDefinitionRefusal::InvalidPrefix)?;
            if value.decimal_exponents & bit != 0 {
                return Err(PhysicalDefinitionRefusal::InvalidPrefix);
            }
            value.decimal_exponents |= bit;
        }
        for &exponent in binary {
            if exponent == 0 || exponent > 80 {
                return Err(PhysicalDefinitionRefusal::InvalidPrefix);
            }
            let bit = 1_u128 << (exponent - 1);
            if value.binary_exponents & bit != 0 {
                return Err(PhysicalDefinitionRefusal::InvalidPrefix);
            }
            value.binary_exponents |= bit;
        }
        Ok(value)
    }
    pub const fn admits_decimal(self, exponent: i8) -> bool {
        match decimal_bit(exponent) {
            Some(bit) => self.decimal_exponents & bit != 0,
            None => false,
        }
    }
    pub const fn admits_binary(self, exponent: u8) -> bool {
        exponent > 0 && exponent <= 80 && (self.binary_exponents & (1_u128 << (exponent - 1))) != 0
    }
    pub fn identity(self) -> [u8; 32] {
        let mut bytes = [0; 25];
        bytes[..8].copy_from_slice(&self.decimal_exponents.to_le_bytes());
        bytes[8..24].copy_from_slice(&self.binary_exponents.to_le_bytes());
        bytes[24] = self.power;
        identity("physical/prefix-policy@1", &bytes)
    }
}
const fn decimal_bit(exponent: i8) -> Option<u64> {
    if exponent == 0 || exponent < -30 || exponent > 30 {
        None
    } else {
        Some(
            1_u64
                << if exponent < 0 {
                    (-exponent - 1) as u32
                } else {
                    (exponent + 29) as u32
                },
        )
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnitPrefix {
    None,
    Decimal(i8),
    Binary(u8),
}
const FAMILY_START: usize = 1;
const SYMBOL_START: usize = FAMILY_START + QUANTITY_FAMILY_ENCODED_LEN;
const POLICY_START: usize = SYMBOL_START + DEFINITION_NAME_ENCODED_LEN;
const PREFIX_START: usize = POLICY_START + 25;
const ROLE_START: usize = PREFIX_START + 3;
const ANCHOR_START: usize = ROLE_START + 1;
const PROFILE_START: usize = ANCHOR_START + 32;
const LENGTH_START: usize = PROFILE_START + PortableNumericProfile::ENCODED_LEN;
const LAW_START: usize = LENGTH_START + 8;
pub const MAX_UNIT_DEFINITION_BYTES: usize = LAW_START + 2 * crate::MAX_NUMERIC_ENCODED_BYTES + 32;

#[derive(Debug, Clone)]
enum CanonicalCapsule {
    Static(&'static [u8]),
    Owned(Box<[u8]>),
}
impl CanonicalCapsule {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Static(value) => value,
            Self::Owned(value) => value,
        }
    }
}
/// Prepared ownership of a checked immutable unit. Static and owned capsules
/// have exactly the same canonical representation and admission law.
#[derive(Debug, Clone)]
pub struct UnitDefinition {
    family: QuantityFamilyDefinition,
    capsule: CanonicalCapsule,
}
impl PartialEq for UnitDefinition {
    fn eq(&self, rhs: &Self) -> bool {
        self.encoded() == rhs.encoded()
    }
}
impl Eq for UnitDefinition {}
impl PartialOrd for UnitDefinition {
    fn partial_cmp(&self, rhs: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for UnitDefinition {
    fn cmp(&self, rhs: &Self) -> core::cmp::Ordering {
        self.encoded().cmp(rhs.encoded())
    }
}
impl core::hash::Hash for UnitDefinition {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.encoded().hash(state);
    }
}
#[derive(Debug, Copy, Clone)]
pub struct UnitDefinitionRef<'a> {
    bytes: &'a [u8],
    family: QuantityFamilyDefinition,
    scale_end: usize,
    offset_end: usize,
}
fn numeric_refusal(error: NumericRefusal) -> PhysicalDefinitionRefusal {
    PhysicalDefinitionRefusal::Numeric(error)
}
fn read_u32(bytes: &[u8], at: usize) -> Result<usize, PhysicalDefinitionRefusal> {
    let slice = bytes
        .get(at..at + 4)
        .ok_or(PhysicalDefinitionRefusal::WrongEncodingLength)?;
    Ok(u32::from_le_bytes(
        slice
            .try_into()
            .map_err(|_| PhysicalDefinitionRefusal::WrongEncodingLength)?,
    ) as usize)
}
fn role_from_byte(byte: u8) -> Result<QuantityRole, PhysicalDefinitionRefusal> {
    match byte {
        0 => Ok(QuantityRole::Linear),
        1 => Ok(QuantityRole::Point),
        2 => Ok(QuantityRole::Delta),
        _ => Err(PhysicalDefinitionRefusal::InvalidRoles),
    }
}
impl<'a> UnitDefinitionRef<'a> {
    pub fn validate(bytes: &'a [u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        if bytes.len() < LAW_START + 52 || bytes.len() > MAX_UNIT_DEFINITION_BYTES {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        if bytes[0] != 2 {
            return Err(PhysicalDefinitionRefusal::UnsupportedVersion);
        }
        let family = QuantityFamilyDefinition::decode(&bytes[FAMILY_START..SYMBOL_START])?;
        DefinitionName::decode(&bytes[SYMBOL_START..POLICY_START])?;
        let power = bytes[POLICY_START + 24];
        let decimal = u64::from_le_bytes(
            bytes[POLICY_START..POLICY_START + 8]
                .try_into()
                .map_err(|_| PhysicalDefinitionRefusal::WrongEncodingLength)?,
        );
        let binary = u128::from_le_bytes(
            bytes[POLICY_START + 8..POLICY_START + 24]
                .try_into()
                .map_err(|_| PhysicalDefinitionRefusal::WrongEncodingLength)?,
        );
        if !(1..=8).contains(&power) || decimal >> 60 != 0 || binary >> 80 != 0 {
            return Err(PhysicalDefinitionRefusal::InvalidPrefix);
        }
        let policy = PrefixPolicy {
            decimal_exponents: decimal,
            binary_exponents: binary,
            power,
        };
        let exponent = i16::from_le_bytes([bytes[PREFIX_START + 1], bytes[PREFIX_START + 2]]);
        match bytes[PREFIX_START] {
            0 if exponent == 0 => {}
            1 if i8::try_from(exponent).is_ok_and(|value| policy.admits_decimal(value)) => {}
            2 if u8::try_from(exponent).is_ok_and(|value| policy.admits_binary(value)) => {}
            _ => return Err(PhysicalDefinitionRefusal::PrefixNotEnabled),
        }
        let role = role_from_byte(bytes[ROLE_START])?;
        if !family.admits(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        let profile = PortableNumericProfile::decode(&bytes[PROFILE_START..LENGTH_START])
            .map_err(numeric_refusal)?;
        let scale_end = LAW_START
            .checked_add(read_u32(bytes, LENGTH_START)?)
            .ok_or(PhysicalDefinitionRefusal::WrongEncodingLength)?;
        let offset_end = scale_end
            .checked_add(read_u32(bytes, LENGTH_START + 4)?)
            .ok_or(PhysicalDefinitionRefusal::WrongEncodingLength)?;
        if offset_end.checked_add(32) != Some(bytes.len()) {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        let scale = profile
            .validate(
                bytes
                    .get(LAW_START..scale_end)
                    .ok_or(PhysicalDefinitionRefusal::WrongEncodingLength)?,
            )
            .map_err(numeric_refusal)?;
        let offset = profile
            .validate(
                bytes
                    .get(scale_end..offset_end)
                    .ok_or(PhysicalDefinitionRefusal::WrongEncodingLength)?,
            )
            .map_err(numeric_refusal)?;
        // Nonzero and affine-role facts are checked from canonical wire without
        // constructing owned bignums or expression graphs.
        if !scale.proven_positive() {
            return Err(PhysicalDefinitionRefusal::InvalidTransform);
        }
        if role != QuantityRole::Point && !offset.is_zero() {
            return Err(PhysicalDefinitionRefusal::InvalidTransform);
        }
        if crate::semantic_digest_parts(
            "physical/unit-definition@2",
            &[&bytes[..PROFILE_START], &bytes[LENGTH_START..offset_end]],
        ) != bytes[offset_end..]
        {
            return Err(PhysicalDefinitionRefusal::IdentityMismatch);
        }
        Ok(Self {
            bytes,
            family,
            scale_end,
            offset_end,
        })
    }
    pub fn encoded(self) -> &'a [u8] {
        self.bytes
    }
    pub const fn family(self) -> QuantityFamilyDefinition {
        self.family
    }
    pub fn symbol(self) -> &'a str {
        core::str::from_utf8(
            &self.bytes[SYMBOL_START + 1..SYMBOL_START + 1 + usize::from(self.bytes[SYMBOL_START])],
        )
        .expect("admitted name")
    }
    pub fn declared_role(self) -> QuantityRole {
        role_from_byte(self.bytes[ROLE_START]).expect("admitted role")
    }
    pub fn reference_anchor(self) -> [u8; 32] {
        self.bytes[ANCHOR_START..PROFILE_START]
            .try_into()
            .expect("admitted anchor")
    }
    pub fn identity(self) -> [u8; 32] {
        self.bytes[self.offset_end..]
            .try_into()
            .expect("admitted identity")
    }
    pub fn law_profile(self) -> PortableNumericProfile {
        PortableNumericProfile::decode(&self.bytes[PROFILE_START..LENGTH_START])
            .expect("admitted profile")
    }
    /// A delta declaration supplies no point-origin law. A point declaration in
    /// a paired family also admits its exact scale-only derivative role.
    pub fn admits_role(self,role:QuantityRole)->bool {
        self.family().admits(role) && (role==self.declared_role()
            || (self.declared_role()==QuantityRole::Point && role==QuantityRole::Delta))
    }
    pub fn validate_profile(
        self,
        required: PortableNumericProfile,
    ) -> Result<(), PhysicalDefinitionRefusal> {
        required.checked().map_err(numeric_refusal)?;
        if !required.admits_profile(self.law_profile()) {
            return Err(numeric_refusal(NumericRefusal::ProfileViolation));
        }
        required
            .validate(self.exact_scale().encoded())
            .map_err(numeric_refusal)?;
        required
            .validate(self.exact_point_offset().encoded())
            .map_err(numeric_refusal)?;
        Ok(())
    }
    pub fn exact_scale(self) -> MathValueRef<'a> {
        MathValueRef::validate(&self.bytes[LAW_START..self.scale_end]).expect("admitted scale")
    }
    pub fn exact_point_offset(self) -> MathValueRef<'a> {
        MathValueRef::validate(&self.bytes[self.scale_end..self.offset_end])
            .expect("admitted offset")
    }
    pub fn same_physical_definition(self, other: Self) -> bool {
        self.family().identity() == other.family().identity()
            && self.declared_role() == other.declared_role()
            && self.reference_anchor() == other.reference_anchor()
            && self.exact_scale().encoded() == other.exact_scale().encoded()
            && self.exact_point_offset().encoded() == other.exact_point_offset().encoded()
    }
    pub fn to_owned(self) -> UnitDefinition {
        UnitDefinition {
            family: self.family,
            capsule: CanonicalCapsule::Owned(self.bytes.into()),
        }
    }
}
struct UnitDefinitionParts<'a> {
    family: QuantityFamilyDefinition,
    symbol: DefinitionName,
    scale: &'a [u8],
    offset: &'a [u8],
    policy: PrefixPolicy,
    prefix: UnitPrefix,
    role: QuantityRole,
    anchor: [u8; 32],
    profile: PortableNumericProfile,
}
impl UnitDefinition {
    pub(crate) const fn from_generated(bytes: &'static [u8]) -> Self {
        Self {
            family: QuantityFamilyDefinition::from_generated(bytes, FAMILY_START),
            capsule: CanonicalCapsule::Static(bytes),
        }
    }
    pub fn encoded(&self) -> &[u8] {
        self.capsule.bytes()
    }
    /// Retained carrier storage including this inline owner; static source capsules have no heap charge.
    pub fn owned_storage_bytes(&self) -> usize {
        core::mem::size_of::<Self>() + self.allocated_storage_bytes()
    }
    /// Heap bytes owned by this capsule, excluding the inline owner and allocator bookkeeping.
    pub fn allocated_storage_bytes(&self) -> usize {
        match &self.capsule {
            CanonicalCapsule::Static(_) => 0,
            CanonicalCapsule::Owned(bytes) => bytes.len(),
        }
    }

    pub fn as_ref(&self) -> UnitDefinitionRef<'_> {
        UnitDefinitionRef::validate(self.encoded()).expect("checked unit ownership")
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        Ok(UnitDefinitionRef::validate(bytes)?.to_owned())
    }
    pub fn encode(&self) -> Vec<u8> {
        self.encoded().to_vec()
    }
    pub fn encode_into(&self, out: &mut [u8]) -> Result<usize, PhysicalDefinitionRefusal> {
        if out.len() < self.encoded().len() {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        out[..self.encoded().len()].copy_from_slice(self.encoded());
        Ok(self.encoded().len())
    }
    pub const fn family(&self) -> QuantityFamilyDefinition {
        self.family
    }
    pub fn symbol(&self) -> &str {
        self.as_ref().symbol()
    }
    pub fn declared_role(&self) -> QuantityRole {
        self.as_ref().declared_role()
    }
    pub fn identity(&self) -> [u8; 32] {
        self.as_ref().identity()
    }
    pub fn reference_anchor(&self) -> [u8; 32] {
        self.as_ref().reference_anchor()
    }
    pub fn exact_scale(&self) -> MathValueRef<'_> {
        self.as_ref().exact_scale()
    }
    pub fn exact_point_offset(&self) -> MathValueRef<'_> {
        self.as_ref().exact_point_offset()
    }
}
impl UnitDefinition {
    pub fn new_math_role(
        family: QuantityFamilyDefinition,
        symbol: &str,
        scale: MathValue,
        offset: MathValue,
        policy: PrefixPolicy,
        role: QuantityRole,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let scale_bytes = scale.canonical_bytes().map_err(numeric_refusal)?;
        let offset_bytes = offset.canonical_bytes().map_err(numeric_refusal)?;
        let profile = PortableNumericProfile::for_value(
            MathValueRef::validate(&scale_bytes).map_err(numeric_refusal)?,
        )
        .union(PortableNumericProfile::for_value(
            MathValueRef::validate(&offset_bytes).map_err(numeric_refusal)?,
        ))
        .map_err(numeric_refusal)?;
        Self::new_math_role_profile(family, symbol, scale, offset, policy, role, profile)
    }
    pub fn related_math_role(
        reference: &Self,
        symbol: &str,
        scale: MathValue,
        offset: MathValue,
        policy: PrefixPolicy,
        role: QuantityRole,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let value = Self::related_math_role_profile(
            reference,
            symbol,
            scale,
            offset,
            policy,
            role,
            PortableNumericProfile::MAXIMAL,
        )?;
        value.reprofile(value.actual_law_profile()?)
    }
    pub fn actual_law_profile(&self) -> Result<PortableNumericProfile, PhysicalDefinitionRefusal> {
        PortableNumericProfile::for_value(self.exact_scale())
            .union(PortableNumericProfile::for_value(self.exact_point_offset()))
            .map_err(numeric_refusal)
    }
    pub fn reprofile(
        &self,
        profile: PortableNumericProfile,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        Self::from_parts(UnitDefinitionParts {
            family: self.family,
            symbol: DefinitionName::new(self.symbol())?,
            scale: self.exact_scale().encoded(),
            offset: self.exact_point_offset().encoded(),
            policy: self.prefix_policy(),
            prefix: self.prefix(),
            role: self.declared_role(),
            anchor: self.reference_anchor(),
            profile,
        })
    }
    pub fn new_math_role_profile(
        family: QuantityFamilyDefinition,
        symbol: &str,
        scale: MathValue,
        offset: MathValue,
        policy: PrefixPolicy,
        role: QuantityRole,
        profile: PortableNumericProfile,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let name = DefinitionName::new(symbol)?;
        if !family.admits(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        let scale = scale.canonical_bytes().map_err(numeric_refusal)?;
        let offset = offset.canonical_bytes().map_err(numeric_refusal)?;
        profile.validate(&scale).map_err(numeric_refusal)?;
        profile.validate(&offset).map_err(numeric_refusal)?;
        let mut root = Vec::new();
        root.extend_from_slice(&family.encode());
        root.extend_from_slice(name.as_str().as_bytes());
        root.extend_from_slice(&(scale.len() as u32).to_le_bytes());
        root.extend_from_slice(&scale);
        root.extend_from_slice(&(offset.len() as u32).to_le_bytes());
        root.extend_from_slice(&offset);
        let anchor = identity("physical/reference-origin@2", &root);
        Self::from_parts(UnitDefinitionParts {
            family,
            symbol: name,
            scale: &scale,
            offset: &offset,
            policy,
            prefix: UnitPrefix::None,
            role,
            anchor,
            profile,
        })
    }
    fn from_parts(parts: UnitDefinitionParts<'_>) -> Result<Self, PhysicalDefinitionRefusal> {
        let UnitDefinitionParts {
            family,
            symbol,
            scale,
            offset,
            policy,
            prefix,
            role,
            anchor,
            profile,
        } = parts;
        let mut bytes = Vec::with_capacity(LAW_START + scale.len() + offset.len() + 32);
        bytes.push(2);
        bytes.extend_from_slice(&family.encode());
        let mut name = [0; DEFINITION_NAME_ENCODED_LEN];
        symbol.encode_into(&mut name);
        bytes.extend_from_slice(&name);
        bytes.extend_from_slice(&policy.decimal_exponents.to_le_bytes());
        bytes.extend_from_slice(&policy.binary_exponents.to_le_bytes());
        bytes.push(policy.power);
        let (tag, exponent) = match prefix {
            UnitPrefix::None => (0, 0),
            UnitPrefix::Decimal(value) => (1, i16::from(value)),
            UnitPrefix::Binary(value) => (2, i16::from(value)),
        };
        bytes.push(tag);
        bytes.extend_from_slice(&exponent.to_le_bytes());
        bytes.push(role as u8);
        bytes.extend_from_slice(&anchor);
        bytes.extend_from_slice(&profile.encode().map_err(numeric_refusal)?);
        bytes.extend_from_slice(&(scale.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(offset.len() as u32).to_le_bytes());
        bytes.extend_from_slice(scale);
        bytes.extend_from_slice(offset);
        let digest = crate::semantic_digest_parts(
            "physical/unit-definition@2",
            &[&bytes[..PROFILE_START], &bytes[LENGTH_START..]],
        );
        bytes.extend_from_slice(&digest);
        UnitDefinitionRef::validate(&bytes)?;
        Ok(Self {
            family,
            capsule: CanonicalCapsule::Owned(bytes.into_boxed_slice()),
        })
    }
    pub fn admits_role(&self,role:QuantityRole)->bool {self.as_ref().admits_role(role)}
    pub fn related_math_role_profile(
        reference: &Self,
        symbol: &str,
        scale: MathValue,
        offset: MathValue,
        policy: PrefixPolicy,
        role: QuantityRole,
        profile: PortableNumericProfile,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if !reference.admits_role(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        let reference_scale = reference
            .exact_scale()
            .to_owned()
            .map_err(numeric_refusal)?;
        let reference_offset = if role == QuantityRole::Point {
            reference
                .exact_point_offset()
                .to_owned()
                .map_err(numeric_refusal)?
        } else {
            MathValue::ZERO
        };
        let composed_offset = offset
            .checked_mul(&reference_scale)
            .and_then(|v| v.checked_add(&reference_offset))
            .map_err(numeric_refusal)?;
        let composed_scale = scale
            .checked_mul(&reference_scale)
            .map_err(numeric_refusal)?;
        let scale = composed_scale.canonical_bytes().map_err(numeric_refusal)?;
        let offset = composed_offset.canonical_bytes().map_err(numeric_refusal)?;
        Self::from_parts(UnitDefinitionParts {
            family: reference.family,
            symbol: DefinitionName::new(symbol)?,
            scale: &scale,
            offset: &offset,
            policy,
            prefix: UnitPrefix::None,
            role,
            anchor: reference.reference_anchor(),
            profile,
        })
    }
    pub fn prefix_policy(&self) -> PrefixPolicy {
        let bytes = self.encoded();
        PrefixPolicy {
            decimal_exponents: u64::from_le_bytes(
                bytes[POLICY_START..POLICY_START + 8]
                    .try_into()
                    .expect("checked policy"),
            ),
            binary_exponents: u128::from_le_bytes(
                bytes[POLICY_START + 8..POLICY_START + 24]
                    .try_into()
                    .expect("checked policy"),
            ),
            power: bytes[POLICY_START + 24],
        }
    }
    pub fn prefix(&self) -> UnitPrefix {
        let bytes = self.encoded();
        let exponent = i16::from_le_bytes([bytes[PREFIX_START + 1], bytes[PREFIX_START + 2]]);
        match bytes[PREFIX_START] {
            0 => UnitPrefix::None,
            1 => UnitPrefix::Decimal(exponent as i8),
            _ => UnitPrefix::Binary(exponent as u8),
        }
    }
    pub fn with_symbol_alias(&self, symbol: &str) -> Result<Self, PhysicalDefinitionRefusal> {
        Self::from_parts(UnitDefinitionParts {
            family: self.family,
            symbol: DefinitionName::new(symbol)?,
            scale: self.exact_scale().encoded(),
            offset: self.exact_point_offset().encoded(),
            policy: self.prefix_policy(),
            prefix: self.prefix(),
            role: self.declared_role(),
            anchor: self.reference_anchor(),
            profile: self.as_ref().law_profile(),
        })
    }
    pub fn with_reference_origin(
        &self,
        reference: &Self,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if self.family.identity() != reference.family.identity() {
            return Err(PhysicalDefinitionRefusal::IncompatibleFamily);
        }
        Self::from_parts(UnitDefinitionParts {
            family: self.family,
            symbol: DefinitionName::new(self.symbol())?,
            scale: self.exact_scale().encoded(),
            offset: self.exact_point_offset().encoded(),
            policy: self.prefix_policy(),
            prefix: self.prefix(),
            role: self.declared_role(),
            anchor: reference.reference_anchor(),
            profile: self.as_ref().law_profile(),
        })
    }
    pub fn with_named_origin(&self, name: &str) -> Result<Self, PhysicalDefinitionRefusal> {
        let name = DefinitionName::new(name)?;
        let mut payload = Vec::new();
        payload.extend_from_slice(&self.reference_anchor());
        payload.extend_from_slice(name.as_str().as_bytes());
        let anchor = identity("physical/named-reference-origin@2", &payload);
        Self::from_parts(UnitDefinitionParts {
            family: self.family,
            symbol: DefinitionName::new(self.symbol())?,
            scale: self.exact_scale().encoded(),
            offset: self.exact_point_offset().encoded(),
            policy: self.prefix_policy(),
            prefix: self.prefix(),
            role: self.declared_role(),
            anchor,
            profile: self.as_ref().law_profile(),
        })
    }
    pub fn with_decimal_prefix(
        &self,
        symbol: &str,
        exponent: i8,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        self.with_prefix(symbol, UnitPrefix::Decimal(exponent))
    }
    pub fn with_binary_prefix(
        &self,
        symbol: &str,
        exponent: u8,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        self.with_prefix(symbol, UnitPrefix::Binary(exponent))
    }
    fn with_prefix(
        &self,
        symbol: &str,
        prefix: UnitPrefix,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if self.prefix() != UnitPrefix::None {
            return Err(PhysicalDefinitionRefusal::AlreadyPrefixed);
        }
        let policy = self.prefix_policy();
        let factor = match prefix {
            UnitPrefix::Decimal(value) if policy.admits_decimal(value) => {
                MathValue::integer(10).checked_pow(i64::from(value) * i64::from(policy.power))
            }
            UnitPrefix::Binary(value) if policy.admits_binary(value) => {
                MathValue::integer(2).checked_pow(i64::from(value) * i64::from(policy.power))
            }
            _ => return Err(PhysicalDefinitionRefusal::PrefixNotEnabled),
        }
        .map_err(numeric_refusal)?;
        let scale = self
            .exact_scale()
            .to_owned()
            .and_then(|value| value.checked_mul(&factor))
            .and_then(|value| value.canonical_bytes())
            .map_err(numeric_refusal)?;
        {
            let profile = PortableNumericProfile::for_value(
                MathValueRef::validate(&scale).map_err(numeric_refusal)?,
            )
            .union(PortableNumericProfile::for_value(self.exact_point_offset()))
            .map_err(numeric_refusal)?;
            Self::from_parts(UnitDefinitionParts {
                family: self.family,
                symbol: DefinitionName::new(symbol)?,
                scale: &scale,
                offset: self.exact_point_offset().encoded(),
                policy,
                prefix,
                role: self.declared_role(),
                anchor: self.reference_anchor(),
                profile,
            })
        }
    }
    pub fn same_physical_definition(&self, rhs: &Self) -> bool {
        self.family.identity() == rhs.family.identity()
            && self.declared_role() == rhs.declared_role()
            && self.reference_anchor() == rhs.reference_anchor()
            && self.exact_scale().encoded() == rhs.exact_scale().encoded()
            && self.exact_point_offset().encoded() == rhs.exact_point_offset().encoded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_source_generated_mathematical_capsules_are_admitted() {
        for (symbol, unit) in crate::BUILTIN_UNIT_DEFINITIONS {
            let checked = UnitDefinitionRef::validate(unit.encoded())
                .unwrap_or_else(|error| panic!("{symbol}: {error:?}"));
            assert_eq!(checked.family(), unit.family());
            assert!(checked.exact_scale().proven_positive());
            assert!(unit.encoded().len() <= MAX_UNIT_DEFINITION_BYTES);
        }
        let turn = &crate::BUILTIN_TURN;
        let rad = &crate::BUILTIN_RADIAN;
        assert_eq!(turn.reference_anchor(), rad.reference_anchor());
        let expected = MathValue::ONE
            .checked_div(&MathValue::integer(2).checked_mul(&MathValue::pi()).unwrap())
            .unwrap();
        assert_eq!(rad.exact_scale().to_owned().unwrap(), expected);
        assert_eq!(
            crate::BUILTIN_RADIAN_PER_SECOND
                .exact_scale()
                .to_owned()
                .unwrap(),
            expected
        );
        const {
            assert!(MAX_UNIT_DEFINITION_BYTES < 131072);
        }
    }
}
