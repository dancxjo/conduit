//! Intrinsic role kinds preserve declaration identity through ordinary leaf validation.
use super::*;
use alloc::string::String;
use core::fmt::Write;
const PREFIX: &str = "quantity/role/";
pub fn quantity_role_info_id(
    family: QuantityFamilyDefinition,
    role: QuantityRole,
) -> Result<String, PhysicalDefinitionRefusal> {
    if !family.admits(role) {
        return Err(PhysicalDefinitionRefusal::InvalidRoles);
    }
    let mut result = String::from(PREFIX);
    for byte in family.identity() {
        write!(result, "{byte:02x}").expect("String write");
    }
    result.push('/');
    result.push_str(match role {
        QuantityRole::Linear => "linear",
        QuantityRole::Point => "point",
        QuantityRole::Delta => "delta",
    });
    result.push_str("@1");
    Ok(result)
}
pub fn parse_quantity_role_info_id(kind: &str) -> Option<([u8; 32], QuantityRole)> {
    let suffix = kind.strip_prefix(PREFIX)?;
    let bytes = suffix.as_bytes();
    if bytes.len() < 66 || bytes[64] != b'/' {
        return None;
    }
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
    let mut identity = [0; 32];
    for (index, item) in identity.iter_mut().enumerate() {
        *item = (hex(bytes[index * 2])? << 4) | hex(bytes[index * 2 + 1])?;
    }
    let role = match &suffix[65..] {
        "linear@1" => QuantityRole::Linear,
        "point@1" => QuantityRole::Point,
        "delta@1" => QuantityRole::Delta,
        _ => return None,
    };
    Some((identity, role))
}
pub const fn builtin_temperature_family() -> QuantityFamilyDefinition {
    BUILTIN_KELVIN.family()
}
pub fn temperature_delta_info_id() -> String {
    quantity_role_info_id(builtin_temperature_family(), QuantityRole::Delta)
        .expect("source-declared paired temperature role")
}

/// Canonical Angle coordinate comes from the source-declared default origin.
pub const fn builtin_angle_unit() -> crate::Unit {
    crate::Unit::from_definition(BUILTIN_ANGLE_DEFAULT_UNIT)
}
