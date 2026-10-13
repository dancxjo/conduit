//! Intrinsic physical transport bounds, separate from family ontology and accuracy.
use super::*;
use crate::{NumericRefusal, PortableNumericProfile};
use alloc::string::String;
use core::fmt::Write;
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PhysicalNumericProfile {
    pub coordinate: PortableNumericProfile,
    pub unit_law: PortableNumericProfile,
}
impl PhysicalNumericProfile {
    pub const MAXIMAL: Self = Self {
        coordinate: PortableNumericProfile::MAXIMAL,
        unit_law: PortableNumericProfile::MAXIMAL,
    };
    pub fn checked(self) -> Result<Self, NumericRefusal> {
        self.coordinate.checked()?;
        self.unit_law.checked()?;
        Ok(self)
    }
    pub fn admits_profile(self, actual: Self) -> bool {
        self.coordinate.admits_profile(actual.coordinate)
            && self.unit_law.admits_profile(actual.unit_law)
    }
    /// Full captured descriptor extent, including bounded family/name custody.
    /// The mathematical law is sized by this profile, not the generic ceiling.
    pub fn maximum_unit_encoded_bytes(self) -> Result<usize, NumericRefusal> {
        self.checked()?;
        Ok(
            crate::MAX_UNIT_DEFINITION_BYTES - 2 * crate::MAX_NUMERIC_ENCODED_BYTES
                + 2 * self.unit_law.maximum_encoded_bytes as usize,
        )
    }
    pub fn maximum_quantity_encoded_bytes(self) -> Result<usize, NumericRefusal> {
        self.checked()?;
        Ok(
            crate::quantity::numeric_quantity::MAX_QUANTITY_ENCODED_BYTES
                - crate::MAX_NUMERIC_ENCODED_BYTES
                - crate::MAX_UNIT_DEFINITION_BYTES
                + self.coordinate.maximum_encoded_bytes as usize
                + self.maximum_unit_encoded_bytes()?,
        )
    }
    pub fn generic_info_id(self) -> Result<String, NumericRefusal> {
        self.checked()?;
        let mut out = String::from("quantity/profile-any/");
        append_profile(&mut out, self.coordinate)?;
        out.push('/');
        append_profile(&mut out, self.unit_law)?;
        out.push_str("@2");
        Ok(out)
    }
    pub fn parse_generic_info_id(kind: &str) -> Option<Self> {
        let mut parts = kind
            .strip_prefix("quantity/profile-any/")?
            .strip_suffix("@2")?
            .split('/');
        let coordinate = parse_profile(parts.next()?)?;
        let unit_law = parse_profile(parts.next()?)?;
        if parts.next().is_some() {
            return None;
        }
        Self {
            coordinate,
            unit_law,
        }
        .checked()
        .ok()
    }
    pub fn info_id(
        self,
        family: QuantityFamilyDefinition,
        role: QuantityRole,
    ) -> Result<String, PhysicalDefinitionRefusal> {
        if !family.admits(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        self.info_id_for_identity(family.identity(), role)
    }
    /// Selects transport bounds for an already checked family/role identity; it does not admit a family declaration.
    pub fn info_id_for_identity(
        self,
        identity: [u8; 32],
        role: QuantityRole,
    ) -> Result<String, PhysicalDefinitionRefusal> {
        self.checked().map_err(PhysicalDefinitionRefusal::Numeric)?;
        let mut out = String::from("qty/profile/");
        for byte in identity {
            write!(out, "{byte:02x}").expect("String write");
        }
        out.push('/');
        out.push_str(match role {
            QuantityRole::Linear => "linear",
            QuantityRole::Point => "point",
            QuantityRole::Delta => "delta",
        });
        for profile in [self.coordinate, self.unit_law] {
            out.push('/');
            for byte in profile
                .encode()
                .map_err(PhysicalDefinitionRefusal::Numeric)?
            {
                write!(out, "{byte:02x}").expect("String write");
            }
        }
        out.push_str("@2");
        Ok(out)
    }
    pub fn parse_info_id(kind: &str) -> Option<([u8; 32], QuantityRole, Self)> {
        let text = kind.strip_prefix("qty/profile/")?.strip_suffix("@2")?;
        let mut fields = text.split('/');
        fn hex<const N: usize>(text: &str) -> Option<[u8; N]> {
            if text.len() != N * 2 {
                return None;
            }
            fn digit(c: u8) -> Option<u8> {
                match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'a'..=b'f' => Some(c - b'a' + 10),
                    _ => None,
                }
            }
            let mut out = [0; N];
            for (index, item) in out.iter_mut().enumerate() {
                *item = (digit(text.as_bytes()[index * 2])? << 4)
                    | digit(text.as_bytes()[index * 2 + 1])?;
            }
            Some(out)
        }
        let identity = hex::<32>(fields.next()?)?;
        let role = match fields.next()? {
            "linear" => QuantityRole::Linear,
            "point" => QuantityRole::Point,
            "delta" => QuantityRole::Delta,
            _ => return None,
        };
        let coordinate = PortableNumericProfile::decode(&hex::<10>(fields.next()?)?).ok()?;
        let unit_law = PortableNumericProfile::decode(&hex::<10>(fields.next()?)?).ok()?;
        if fields.next().is_some() {
            return None;
        }
        Some((
            identity,
            role,
            Self {
                coordinate,
                unit_law,
            }
            .checked()
            .ok()?,
        ))
    }
}
fn append_profile(out: &mut String, profile: PortableNumericProfile) -> Result<(), NumericRefusal> {
    for byte in profile.encode()? {
        write!(out, "{byte:02x}").expect("String write");
    }
    Ok(())
}
fn parse_profile(text: &str) -> Option<PortableNumericProfile> {
    if text.len() != 20 {
        return None;
    }
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        }
    }
    let mut bytes = [0; 10];
    for (i, item) in bytes.iter_mut().enumerate() {
        *item = (hex(text.as_bytes()[2 * i])? << 4) | hex(text.as_bytes()[2 * i + 1])?;
    }
    PortableNumericProfile::decode(&bytes).ok()
}
pub fn unit_numeric_profile_info_id(
    profile: PortableNumericProfile,
) -> Result<String, NumericRefusal> {
    let mut out = String::from("unit/profile/");
    append_profile(&mut out, profile)?;
    out.push_str("@2");
    Ok(out)
}
pub fn parse_unit_numeric_profile_info_id(kind: &str) -> Option<PortableNumericProfile> {
    parse_profile(kind.strip_prefix("unit/profile/")?.strip_suffix("@2")?)
}
/// Intrinsic byte extent for selected mathematical and physical profiles.
/// Unknown nonnumeric kinds are left to their own primitive contract.
pub fn intrinsic_numeric_maximum_bytes(kind: &str) -> Option<usize> {
    if kind == crate::MATH_VALUE_INFO_ID {
        return Some(crate::MAX_NUMERIC_ENCODED_BYTES);
    }
    if let Some(profile) = PortableNumericProfile::from_info_id(kind) {
        return Some(profile.maximum_encoded_bytes as usize);
    }
    if kind == crate::QUANTITY_INFO_ID || crate::parse_quantity_role_info_id(kind).is_some() {
        return Some(crate::MAX_QUANTITY_ENCODED_BYTES);
    }
    if kind == crate::UNIT_INFO_ID {
        return Some(crate::MAX_UNIT_ENCODED_BYTES);
    }
    if let Some(profile) = parse_unit_numeric_profile_info_id(kind) {
        return PhysicalNumericProfile {
            coordinate: profile,
            unit_law: profile,
        }
        .maximum_unit_encoded_bytes()
        .ok();
    }
    PhysicalNumericProfile::parse_generic_info_id(kind)
        .or_else(|| PhysicalNumericProfile::parse_info_id(kind).map(|(_, _, profile)| profile))?
        .maximum_quantity_encoded_bytes()
        .ok()
}
