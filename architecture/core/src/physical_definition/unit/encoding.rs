//! Canonical Unit definition encoding, exact comparison and checked decoding.
use super::*;

impl UnitDefinition {
    pub fn encode(self) -> [u8; UNIT_DEFINITION_ENCODED_LEN] {
        let mut bytes = [0; UNIT_DEFINITION_ENCODED_LEN];
        let mut at = 0;
        self.write_encoding(&mut |part| {
            bytes[at..at + part.len()].copy_from_slice(part);
            at += part.len();
        });
        bytes
    }
    fn write_encoding(&self, write: &mut impl FnMut(&[u8])) {
        write(&[1]);
        self.family.write_encoding(write);
        self.symbol.write_encoding(write);
        for value in [self.scale, self.offset, self.denominator] {
            write(&value.to_le_bytes());
        }
        write(&self.decimal_exponent.to_le_bytes());
        write(&self.offset_exponent.to_le_bytes());
        write(&self.policy.decimal_exponents.to_le_bytes());
        write(&self.policy.binary_exponents.to_le_bytes());
        write(&[self.policy.power]);
        let (kind, exponent) = match self.prefix {
            UnitPrefix::None => (0, 0),
            UnitPrefix::Decimal(p) => (1, p as u8),
            UnitPrefix::Binary(p) => (2, p),
        };
        write(&[kind, exponent, 0]);
        write(&[match self.declared_role {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        }]);
        write(&self.reference_anchor);
        write(&self.identity);
    }
    /// Compare every canonical byte without materializing an encoded definition.
    pub(crate) fn matches_encoding(&self, bytes: &[u8]) -> bool {
        if bytes.len() != UNIT_DEFINITION_ENCODED_LEN {
            return false;
        }
        let mut at = 0;
        let mut matches = true;
        self.write_encoding(&mut |part| {
            matches &= bytes[at..at + part.len()] == *part;
            at += part.len();
        });
        matches && at == bytes.len()
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
