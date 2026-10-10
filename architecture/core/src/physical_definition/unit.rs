use super::*;
pub const UNIT_DEFINITION_ENCODED_LEN: usize =
    1 + QUANTITY_FAMILY_ENCODED_LEN + DEFINITION_NAME_ENCODED_LEN + 48 + 4 + 25 + 3 + 1 + 32 + 32;
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
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitDefinition {
    family: QuantityFamilyDefinition,
    symbol: DefinitionName,
    scale: i128,
    offset: i128,
    denominator: i128,
    decimal_exponent: i16,
    offset_exponent: i16,
    policy: PrefixPolicy,
    prefix: UnitPrefix,
    declared_role: QuantityRole,
    reference_anchor: [u8; 32],
    identity: [u8; 32],
}
impl UnitDefinition {
    pub(crate) const fn from_generated(bytes: &[u8; UNIT_DEFINITION_ENCODED_LEN]) -> Self {
        let symbol_start = 1 + QUANTITY_FAMILY_ENCODED_LEN;
        let transform_start = symbol_start + DEFINITION_NAME_ENCODED_LEN;
        let pos = transform_start + 48;
        let mut dec = [0; 8];
        let mut bin = [0; 16];
        let mut i = 0;
        while i < 16 {
            bin[i] = bytes[pos + 12 + i];
            if i < 8 {
                dec[i] = bytes[pos + 4 + i];
            }
            i += 1;
        }
        Self {
            family: QuantityFamilyDefinition::from_generated(bytes, 1),
            symbol: DefinitionName::from_generated(bytes, symbol_start),
            scale: generated_i128(bytes, transform_start),
            offset: generated_i128(bytes, transform_start + 16),
            denominator: generated_i128(bytes, transform_start + 32),
            decimal_exponent: i16::from_le_bytes([bytes[pos], bytes[pos + 1]]),
            offset_exponent: i16::from_le_bytes([bytes[pos + 2], bytes[pos + 3]]),
            policy: PrefixPolicy {
                decimal_exponents: u64::from_le_bytes(dec),
                binary_exponents: u128::from_le_bytes(bin),
                power: bytes[pos + 28],
            },
            prefix: match bytes[pos + 29] {
                1 => UnitPrefix::Decimal(bytes[pos + 30] as i8),
                2 => UnitPrefix::Binary(bytes[pos + 30]),
                _ => UnitPrefix::None,
            },
            declared_role: match bytes[UNIT_DEFINITION_ENCODED_LEN - 65] {
                0 => QuantityRole::Linear,
                1 => QuantityRole::Point,
                _ => QuantityRole::Delta,
            },
            reference_anchor: generated_digest(bytes, UNIT_DEFINITION_ENCODED_LEN - 64),
            identity: generated_digest(bytes, UNIT_DEFINITION_ENCODED_LEN - 32),
        }
    }
    pub fn new(
        family: QuantityFamilyDefinition,
        symbol: &str,
        scale: i128,
        offset: i128,
        denominator: i128,
        decimal_exponent: i16,
        policy: PrefixPolicy,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if scale <= 0
            || denominator <= 0
            || (family.roles() == QuantityRoles::Linear && offset != 0)
        {
            return Err(PhysicalDefinitionRefusal::InvalidTransform);
        }
        if !(-128..=128).contains(&decimal_exponent) {
            return Err(PhysicalDefinitionRefusal::InvalidExponent);
        }
        if !(1..=8).contains(&policy.power) {
            return Err(PhysicalDefinitionRefusal::InvalidPrefix);
        }
        // A common positive denominator and joint reduction make one exact law canonical.
        let gcd = gcd(
            gcd(scale as u128, offset.unsigned_abs()),
            denominator as u128,
        ) as i128;
        let mut value = Self {
            family,
            symbol: DefinitionName::new(symbol)?,
            scale: scale / gcd,
            offset: offset / gcd,
            denominator: denominator / gcd,
            decimal_exponent,
            offset_exponent: 0,
            policy,
            prefix: UnitPrefix::None,
            declared_role: if family.roles() == QuantityRoles::Linear {
                QuantityRole::Linear
            } else {
                QuantityRole::Point
            },
            reference_anchor: [0; 32],
            identity: [0; 32],
        };
        value.reference_anchor = value.root_reference_anchor();
        value.seal();
        Ok(value)
    }
    pub fn new_exact_role(
        family: QuantityFamilyDefinition,
        symbol: &str,
        role: QuantityRole,
        scale: DefinitionScalar,
        offset: DefinitionScalar,
        policy: PrefixPolicy,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if !family.admits(role) || (role == QuantityRole::Delta && offset.numerator != 0) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        let mut value = Self::new_exact(family, symbol, scale, offset, policy)?;
        value.declared_role = role;
        value.seal();
        Ok(value)
    }
    pub fn related_exact_role(
        reference: Self,
        symbol: &str,
        role: QuantityRole,
        scale: DefinitionScalar,
        offset: DefinitionScalar,
        policy: PrefixPolicy,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let reference_scale = reference.exact_scale();
        let result_scale = scale
            .multiply(reference_scale)?
            .multiply_binary(reference.binary_exponent())?;
        let result_offset = offset
            .multiply(reference_scale)?
            .multiply_binary(reference.binary_exponent())?
            .checked_add(reference.exact_offset(role)?)?;
        let mut value = Self::new_exact_role(
            reference.family,
            symbol,
            role,
            result_scale,
            result_offset,
            policy,
        )?;
        value.reference_anchor = reference.reference_anchor;
        value.seal();
        Ok(value)
    }
    pub fn new_exact(
        family: QuantityFamilyDefinition,
        symbol: &str,
        scale: DefinitionScalar,
        offset: DefinitionScalar,
        policy: PrefixPolicy,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let scale =
            DefinitionScalar::new(scale.numerator, scale.denominator, scale.decimal_exponent)?;
        let offset = DefinitionScalar::new(
            offset.numerator,
            offset.denominator,
            offset.decimal_exponent,
        )?;
        let gcd = super::scalar::scalar_gcd(scale.denominator as u128, offset.denominator as u128)
            as i128;
        let denominator = (scale.denominator / gcd)
            .checked_mul(offset.denominator)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let sn = scale
            .numerator
            .checked_mul(denominator / scale.denominator)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let on = offset
            .numerator
            .checked_mul(denominator / offset.denominator)
            .ok_or(PhysicalDefinitionRefusal::TransformOverflow)?;
        let mut value = Self::new(
            family,
            symbol,
            sn,
            on,
            denominator,
            scale.decimal_exponent,
            policy,
        )?;
        value.offset_exponent = offset.decimal_exponent;
        value.reference_anchor = value.root_reference_anchor();
        value.seal();
        Ok(value)
    }
    pub fn related_exact(
        reference: Self,
        symbol: &str,
        scale: DefinitionScalar,
        offset: DefinitionScalar,
        policy: PrefixPolicy,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let reference_scale = reference.exact_scale();
        let result_scale = scale
            .multiply(reference_scale)?
            .multiply_binary(reference.binary_exponent())?;
        let result_offset = offset
            .multiply(reference_scale)?
            .multiply_binary(reference.binary_exponent())?
            .checked_add(reference.exact_offset(reference.declared_role)?)?;
        let mut value = Self::new_exact(
            reference.family,
            symbol,
            result_scale,
            result_offset,
            policy,
        )?;
        value.reference_anchor = reference.reference_anchor;
        value.seal();
        Ok(value)
    }
    /// Emit an already source-admitted spelling with the same immutable physical law.
    pub fn with_symbol_alias(mut self, symbol: &str) -> Result<Self, PhysicalDefinitionRefusal> {
        self.symbol = DefinitionName::new(symbol)?;
        self.seal();
        Ok(self)
    }
    fn root_reference_anchor(self) -> [u8; 32] {
        let mut bytes = [0; 32 + DEFINITION_NAME_ENCODED_LEN + 82];
        bytes[..32].copy_from_slice(&self.family.identity());
        self.symbol
            .encode_into(&mut bytes[32..32 + DEFINITION_NAME_ENCODED_LEN]);
        let start = 32 + DEFINITION_NAME_ENCODED_LEN;
        bytes[start..start + 41].copy_from_slice(
            &DefinitionScalar {
                numerator: self.scale,
                denominator: self.denominator,
                decimal_exponent: self.decimal_exponent,
            }
            .identity_bytes(),
        );
        bytes[start + 41..].copy_from_slice(
            &DefinitionScalar {
                numerator: self.offset,
                denominator: self.denominator,
                decimal_exponent: self.offset_exponent,
            }
            .identity_bytes(),
        );
        identity("physical/reference-origin@1", &bytes)
    }
    /// Pair an explicitly checked derivative origin with its family's point origin.
    pub fn with_reference_origin(
        mut self,
        reference: Self,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if self.family.identity() != reference.family.identity() {
            return Err(PhysicalDefinitionRefusal::IncompatibleFamily);
        }
        self.reference_anchor = reference.reference_anchor;
        self.seal();
        Ok(self)
    }
    pub fn with_named_origin(mut self, name: &str) -> Result<Self, PhysicalDefinitionRefusal> {
        let name = DefinitionName::new(name)?;
        let mut bytes = [0; 32 + DEFINITION_NAME_ENCODED_LEN];
        bytes[..32].copy_from_slice(&self.root_reference_anchor());
        name.encode_into(&mut bytes[32..]);
        self.reference_anchor = identity("physical/named-reference-origin@1", &bytes);
        self.seal();
        Ok(self)
    }
    pub const fn exact_scale(self) -> DefinitionScalar {
        DefinitionScalar {
            numerator: self.scale,
            denominator: self.denominator,
            decimal_exponent: self.decimal_exponent(),
        }
    }
    pub fn exact_offset(
        self,
        role: QuantityRole,
    ) -> Result<DefinitionScalar, PhysicalDefinitionRefusal> {
        if !self.family.admits(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        DefinitionScalar::new(
            if role == QuantityRole::Delta {
                0
            } else {
                self.offset
            },
            self.denominator,
            if role == QuantityRole::Delta {
                0
            } else {
                self.offset_exponent
            },
        )
    }
    /// Resolve an exact authored affine relationship into an immutable reference law.
    /// The relation is `(coordinate * scale + offset) / denominator` in `reference`.
    pub fn related(
        reference: Self,
        symbol: &str,
        scale: i128,
        offset: i128,
        denominator: i128,
        policy: PrefixPolicy,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        Self::related_exact(
            reference,
            symbol,
            DefinitionScalar::new(scale, denominator, 0)?,
            DefinitionScalar::new(offset, denominator, 0)?,
            policy,
        )
    }
    fn seal(&mut self) {
        self.identity = identity(
            "physical/unit-definition@1",
            &self.encode()[..UNIT_DEFINITION_ENCODED_LEN - 32],
        );
    }
    pub fn same_physical_definition(self, other: Self) -> bool {
        self.family.identity() == other.family.identity()
            && self.reference_anchor == other.reference_anchor
            && self.declared_role == other.declared_role
            && self.exact_scale().equivalent_binary(
                self.binary_exponent() as i16,
                other.exact_scale(),
                other.binary_exponent() as i16,
            )
            && self
                .exact_offset(self.declared_role)
                .expect("checked role")
                .equivalent(
                    other
                        .exact_offset(other.declared_role)
                        .expect("checked role"),
                )
    }
    pub const fn family(self) -> QuantityFamilyDefinition {
        self.family
    }
    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }
    pub const fn declared_role(self) -> QuantityRole {
        self.declared_role
    }
    pub const fn reference_anchor(self) -> [u8; 32] {
        self.reference_anchor
    }
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    pub const fn prefix_policy(self) -> PrefixPolicy {
        self.policy
    }
    pub const fn prefix(self) -> UnitPrefix {
        self.prefix
    }
    pub const fn decimal_exponent(self) -> i16 {
        self.decimal_exponent
            + match self.prefix {
                UnitPrefix::Decimal(exponent) => exponent as i16 * self.policy.power as i16,
                _ => 0,
            }
    }
    pub const fn offset_exponent(self) -> i16 {
        self.offset_exponent
    }
    pub const fn binary_exponent(self) -> u16 {
        match self.prefix {
            UnitPrefix::Binary(exponent) => exponent as u16 * self.policy.power as u16,
            _ => 0,
        }
    }
    pub fn with_decimal_prefix(
        self,
        symbol: &str,
        exponent: i8,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if !self.policy.admits_decimal(exponent) {
            return Err(PhysicalDefinitionRefusal::PrefixNotEnabled);
        }
        if exponent == 0 || !(-30..=30).contains(&exponent) {
            return Err(PhysicalDefinitionRefusal::InvalidPrefix);
        }
        self.derive(symbol, UnitPrefix::Decimal(exponent))
    }
    pub fn with_binary_prefix(
        self,
        symbol: &str,
        exponent: u8,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if !self.policy.admits_binary(exponent) {
            return Err(PhysicalDefinitionRefusal::PrefixNotEnabled);
        }
        if exponent == 0 || exponent > 80 {
            return Err(PhysicalDefinitionRefusal::InvalidPrefix);
        }
        self.derive(symbol, UnitPrefix::Binary(exponent))
    }
    fn derive(
        mut self,
        symbol: &str,
        prefix: UnitPrefix,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if self.prefix != UnitPrefix::None {
            return Err(PhysicalDefinitionRefusal::AlreadyPrefixed);
        }
        self.symbol = DefinitionName::new(symbol)?;
        self.prefix = prefix;
        if !(-128..=128).contains(&self.decimal_exponent()) {
            return Err(PhysicalDefinitionRefusal::InvalidExponent);
        }
        self.seal();
        Ok(self)
    }
    pub fn encode(self) -> [u8; UNIT_DEFINITION_ENCODED_LEN] {
        let mut bytes = [0; UNIT_DEFINITION_ENCODED_LEN];
        bytes[0] = 1;
        let symbol_start = 1 + QUANTITY_FAMILY_ENCODED_LEN;
        let transform_start = symbol_start + DEFINITION_NAME_ENCODED_LEN;
        bytes[1..symbol_start].copy_from_slice(&self.family.encode());
        self.symbol
            .encode_into(&mut bytes[symbol_start..transform_start]);
        for (index, value) in [self.scale, self.offset, self.denominator]
            .iter()
            .enumerate()
        {
            let start = transform_start + index * 16;
            bytes[start..start + 16].copy_from_slice(&value.to_le_bytes());
        }
        let pos = transform_start + 48;
        bytes[pos..pos + 2].copy_from_slice(&self.decimal_exponent.to_le_bytes());
        bytes[pos + 2..pos + 4].copy_from_slice(&self.offset_exponent.to_le_bytes());
        bytes[pos + 4..pos + 12].copy_from_slice(&self.policy.decimal_exponents.to_le_bytes());
        bytes[pos + 12..pos + 28].copy_from_slice(&self.policy.binary_exponents.to_le_bytes());
        bytes[pos + 28] = self.policy.power;
        let (kind, exponent) = match self.prefix {
            UnitPrefix::None => (0, 0),
            UnitPrefix::Decimal(p) => (1, p as u8),
            UnitPrefix::Binary(p) => (2, p),
        };
        bytes[pos + 29] = kind;
        bytes[pos + 30] = exponent; // pos + 31 reserved zero
        bytes[UNIT_DEFINITION_ENCODED_LEN - 65] = match self.declared_role {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        };
        bytes[UNIT_DEFINITION_ENCODED_LEN - 64..UNIT_DEFINITION_ENCODED_LEN - 32]
            .copy_from_slice(&self.reference_anchor);
        bytes[UNIT_DEFINITION_ENCODED_LEN - 32..].copy_from_slice(&self.identity);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        if bytes.len() != UNIT_DEFINITION_ENCODED_LEN {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        if bytes[0] != 1 {
            return Err(PhysicalDefinitionRefusal::UnsupportedVersion);
        }
        let symbol_start = 1 + QUANTITY_FAMILY_ENCODED_LEN;
        let transform_start = symbol_start + DEFINITION_NAME_ENCODED_LEN;
        let pos = transform_start + 48;
        let family = QuantityFamilyDefinition::decode(&bytes[1..symbol_start])?;
        let symbol = DefinitionName::decode(&bytes[symbol_start..transform_start])?;
        let read = |index: usize| {
            i128::from_le_bytes(
                bytes[transform_start + index * 16..transform_start + (index + 1) * 16]
                    .try_into()
                    .unwrap(),
            )
        };
        if bytes[pos + 31] != 0 {
            return Err(PhysicalDefinitionRefusal::NonCanonicalEncoding);
        }
        let policy = PrefixPolicy {
            decimal_exponents: u64::from_le_bytes(bytes[pos + 4..pos + 12].try_into().unwrap()),
            binary_exponents: u128::from_le_bytes(bytes[pos + 12..pos + 28].try_into().unwrap()),
            power: bytes[pos + 28],
        };
        if policy.decimal_exponents >> 60 != 0 || policy.binary_exponents >> 80 != 0 {
            return Err(PhysicalDefinitionRefusal::InvalidPrefix);
        }
        let mut value = Self::new(
            family,
            symbol.as_str(),
            read(0),
            read(1),
            read(2),
            i16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap()),
            policy,
        )?;
        value = match bytes[pos + 29] {
            0 if bytes[pos + 30] == 0 => value,
            1 => value.with_decimal_prefix(symbol.as_str(), bytes[pos + 30] as i8)?,
            2 => value.with_binary_prefix(symbol.as_str(), bytes[pos + 30])?,
            _ => return Err(PhysicalDefinitionRefusal::InvalidPrefix),
        };
        value.declared_role = match bytes[UNIT_DEFINITION_ENCODED_LEN - 65] {
            0 => QuantityRole::Linear,
            1 => QuantityRole::Point,
            2 => QuantityRole::Delta,
            _ => return Err(PhysicalDefinitionRefusal::InvalidRoles),
        };
        if !value.family.admits(value.declared_role)
            || (value.declared_role == QuantityRole::Delta && value.offset != 0)
        {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        value.offset_exponent = i16::from_le_bytes(bytes[pos + 2..pos + 4].try_into().unwrap());
        if !(-128..=128).contains(&value.offset_exponent)
            || (value.offset == 0 && value.offset_exponent != 0)
        {
            return Err(PhysicalDefinitionRefusal::NonCanonicalEncoding);
        }
        value.reference_anchor.copy_from_slice(
            &bytes[UNIT_DEFINITION_ENCODED_LEN - 64..UNIT_DEFINITION_ENCODED_LEN - 32],
        );
        value.seal();
        if value.encode() != bytes {
            return Err(PhysicalDefinitionRefusal::IdentityMismatch);
        }
        Ok(value)
    }
}
fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
