//! Read sealed typed properties as values rather than opaque canonical bytes.
use crate::PlacementAdmission;
use conduit_core::{
    StructuredConfigurationValue, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};
use core::fmt;

impl fmt::Debug for PlacementAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlacementAdmission")
            .field("limits", &self.limits)
            .field(
                "realization_characteristics",
                &self.realization_characteristics,
            )
            .field(
                "realization_properties",
                &Properties(&self.realization_properties),
            )
            .field("pool_references", &self.pool_references)
            .field("terminal_transductions", &self.terminal_transductions)
            .finish()
    }
}
struct Properties<'a>(&'a [StructuredConfigurationValue]);
impl fmt::Debug for Properties<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for property in self.0 {
            // Canonical construction/deserialization already validates this;
            // retain an honest refusal if a future encoding is unsupported.
            match StructuredInfoValue::from_canonical_bytes(property.canonical_value()) {
                Ok(value) => {
                    map.entry(&property.profile().as_str(), &Value(&value));
                }
                Err(error) => {
                    map.entry(&property.profile().as_str(), &error);
                }
            }
        }
        map.finish()
    }
}
struct Value<'a>(&'a StructuredInfoValue);
impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.0.value_type().shape(), self.0.shape()) {
            (StructuredInfoTypeShape::Leaf(kind), StructuredInfoValueShape::Leaf(bytes))
                if kind.as_str() == "value/u64" && bytes.len() == 8 =>
            {
                u64::from_le_bytes(bytes.try_into().map_err(|_| fmt::Error)?).fmt(f)
            }
            (_, StructuredInfoValueShape::Record(fields)) => {
                let mut map = f.debug_map();
                for field in fields {
                    map.entry(&field.name(), &Value(field.value()));
                }
                map.finish()
            }
            // Other Types keep their exact type/shape/payload; do not guess a
            // numeric interpretation for nominal or domain-specific Forms.
            _ => self.0.fmt(f),
        }
    }
}
