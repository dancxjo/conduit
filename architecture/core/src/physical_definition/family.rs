use super::*;
pub const QUANTITY_FAMILY_ENCODED_LEN: usize =
    1 + DEFINITION_NAME_ENCODED_LEN + DIMENSION_DEFINITION_ENCODED_LEN + 65 + 1 + 32;
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuantityRole {
    Linear,
    Point,
    Delta,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuantityRoles {
    Linear,
    PointDelta,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuantityFamilyDefinition {
    name: DefinitionName,
    dimension: DimensionDefinition,
    delta_name: [u8; 64],
    delta_length: u8,
    roles: QuantityRoles,
    identity: [u8; 32],
}
impl QuantityFamilyDefinition {
    pub(crate) const fn from_generated(bytes: &[u8], position: usize) -> Self {
        let dimension_start = position + 1 + DEFINITION_NAME_ENCODED_LEN;
        let delta_start = dimension_start + DIMENSION_DEFINITION_ENCODED_LEN;
        let mut delta_name = [0; 64];
        let mut i = 0;
        while i < 64 {
            delta_name[i] = bytes[delta_start + 1 + i];
            i += 1;
        }
        Self {
            name: DefinitionName::from_generated(bytes, position + 1),
            dimension: DimensionDefinition::from_generated(bytes, dimension_start),
            delta_name,
            delta_length: bytes[delta_start],
            roles: if bytes[position + QUANTITY_FAMILY_ENCODED_LEN - 33] == 0 {
                QuantityRoles::Linear
            } else {
                QuantityRoles::PointDelta
            },
            identity: generated_digest(bytes, position + QUANTITY_FAMILY_ENCODED_LEN - 32),
        }
    }
    pub fn new(
        name: &str,
        dimension: DimensionDefinition,
        roles: QuantityRoles,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if roles != QuantityRoles::Linear {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        Self::build(name, None, dimension)
    }
    pub fn point_delta(
        point_name: &str,
        delta_name: &str,
        dimension: DimensionDefinition,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        if point_name == delta_name {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        Self::build(point_name, Some(delta_name), dimension)
    }
    fn build(
        name: &str,
        delta: Option<&str>,
        dimension: DimensionDefinition,
    ) -> Result<Self, PhysicalDefinitionRefusal> {
        let mut delta_name = [0; 64];
        if let Some(delta) = delta {
            if delta.is_empty() {
                return Err(PhysicalDefinitionRefusal::EmptyName);
            }
            if delta.len() > 64 {
                return Err(PhysicalDefinitionRefusal::NameTooLong);
            }
            delta_name[..delta.len()].copy_from_slice(delta.as_bytes());
        }
        let mut value = Self {
            name: DefinitionName::new(name)?,
            dimension,
            delta_name,
            delta_length: delta.map_or(0, |name| name.len()) as u8,
            roles: if delta.is_some() {
                QuantityRoles::PointDelta
            } else {
                QuantityRoles::Linear
            },
            identity: [0; 32],
        };
        value.identity = identity(
            "physical/quantity-family@1",
            &value.encode()[..QUANTITY_FAMILY_ENCODED_LEN - 32],
        );
        Ok(value)
    }
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
    pub fn role_name(&self, role: QuantityRole) -> Result<&str, PhysicalDefinitionRefusal> {
        if !self.admits(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        if role == QuantityRole::Delta {
            core::str::from_utf8(&self.delta_name[..self.delta_length as usize])
                .map_err(|_| PhysicalDefinitionRefusal::InvalidUtf8)
        } else {
            Ok(self.name())
        }
    }
    pub const fn dimension(self) -> DimensionDefinition {
        self.dimension
    }
    pub const fn roles(self) -> QuantityRoles {
        self.roles
    }
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    pub const fn admits(self, role: QuantityRole) -> bool {
        matches!(
            (self.roles, role),
            (QuantityRoles::Linear, QuantityRole::Linear)
                | (
                    QuantityRoles::PointDelta,
                    QuantityRole::Point | QuantityRole::Delta
                )
        )
    }
    pub fn role_identity(self, role: QuantityRole) -> Result<[u8; 32], PhysicalDefinitionRefusal> {
        if !self.admits(role) {
            return Err(PhysicalDefinitionRefusal::InvalidRoles);
        }
        let mut bytes = [0; 33];
        bytes[..32].copy_from_slice(&self.identity);
        bytes[32] = match role {
            QuantityRole::Linear => 0,
            QuantityRole::Point => 1,
            QuantityRole::Delta => 2,
        };
        Ok(identity("physical/quantity-role@1", &bytes))
    }
    pub fn encode(self) -> [u8; QUANTITY_FAMILY_ENCODED_LEN] {
        let mut bytes = [0; QUANTITY_FAMILY_ENCODED_LEN];
        bytes[0] = 1;
        let dimension_start = 1 + DEFINITION_NAME_ENCODED_LEN;
        self.name.encode_into(&mut bytes[1..dimension_start]);
        bytes[dimension_start..dimension_start + DIMENSION_DEFINITION_ENCODED_LEN]
            .copy_from_slice(&self.dimension.encode());
        let delta_start = dimension_start + DIMENSION_DEFINITION_ENCODED_LEN;
        bytes[delta_start] = self.delta_length;
        bytes[delta_start + 1..delta_start + 65].copy_from_slice(&self.delta_name);
        bytes[QUANTITY_FAMILY_ENCODED_LEN - 33] = match self.roles {
            QuantityRoles::Linear => 0,
            QuantityRoles::PointDelta => 1,
        };
        bytes[QUANTITY_FAMILY_ENCODED_LEN - 32..].copy_from_slice(&self.identity);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, PhysicalDefinitionRefusal> {
        if bytes.len() != QUANTITY_FAMILY_ENCODED_LEN {
            return Err(PhysicalDefinitionRefusal::WrongEncodingLength);
        }
        if bytes[0] != 1 {
            return Err(PhysicalDefinitionRefusal::UnsupportedVersion);
        }
        let start = 1 + DEFINITION_NAME_ENCODED_LEN;
        let name = DefinitionName::decode(&bytes[1..start])?;
        let dimension =
            DimensionDefinition::decode(&bytes[start..start + DIMENSION_DEFINITION_ENCODED_LEN])?;
        let roles = match bytes[QUANTITY_FAMILY_ENCODED_LEN - 33] {
            0 => QuantityRoles::Linear,
            1 => QuantityRoles::PointDelta,
            _ => return Err(PhysicalDefinitionRefusal::InvalidRoles),
        };
        let delta_start = start + DIMENSION_DEFINITION_ENCODED_LEN;
        let delta_length = bytes[delta_start] as usize;
        if delta_length > 64 {
            return Err(PhysicalDefinitionRefusal::NameTooLong);
        }
        if bytes[delta_start + 1 + delta_length..delta_start + 65]
            .iter()
            .any(|b| *b != 0)
        {
            return Err(PhysicalDefinitionRefusal::NonCanonicalEncoding);
        }
        let value = match roles {
            QuantityRoles::Linear if delta_length == 0 => {
                Self::new(name.as_str(), dimension, roles)?
            }
            QuantityRoles::PointDelta => Self::point_delta(
                name.as_str(),
                core::str::from_utf8(&bytes[delta_start + 1..delta_start + 1 + delta_length])
                    .map_err(|_| PhysicalDefinitionRefusal::InvalidUtf8)?,
                dimension,
            )?,
            _ => return Err(PhysicalDefinitionRefusal::InvalidRoles),
        };
        if value.encode() != bytes {
            return Err(PhysicalDefinitionRefusal::IdentityMismatch);
        }
        Ok(value)
    }
}
