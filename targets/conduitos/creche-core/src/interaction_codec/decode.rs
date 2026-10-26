//! Recursive view of packed byte arrays as their original integer sequences.
//! Typed scalar methods delegate directly: Ciborium never gives those visitors
//! child deserializers. Only paths that can expose children require a wrapper.
use super::visit::{Gate, Wrapped};
use serde::de::{self, Visitor};
pub(super) struct Sequences<D> {
    pub inner: D,
    pub depth: u8,
}
impl<'de, D: de::Deserializer<'de>> de::Deserializer<'de> for Sequences<D> {
    type Error = D::Error;
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_any(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Any,
        })
    }
    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_bool(visitor)
    }
    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_i8(visitor)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_i16(visitor)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_i32(visitor)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_i64(visitor)
    }
    fn deserialize_i128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_i128(visitor)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_u8(visitor)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_u16(visitor)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_u32(visitor)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_u64(visitor)
    }
    fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_u128(visitor)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_f32(visitor)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_f64(visitor)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_char(visitor)
    }
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_str(visitor)
    }
    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_string(visitor)
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, _: V) -> Result<V::Value, D::Error> {
        Err(de::Error::custom("source byte strings unsupported"))
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, _: V) -> Result<V::Value, D::Error> {
        Err(de::Error::custom("source byte strings unsupported"))
    }
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_option(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Any,
        })
    }
    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_unit(visitor)
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_identifier(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Identifier,
        })
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_ignored_any(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Any,
        })
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.inner.deserialize_unit_struct(name, visitor)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.inner.deserialize_newtype_struct(
            name,
            Wrapped {
                visitor,
                depth: self.depth,
                gate: Gate::Any,
            },
        )
    }
    // Ciborium's native bytes-as-sequence path emits u8 callbacks and does
    // not charge a container depth. Intercept bytes through any instead.
    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_any(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Seq(None),
        })
    }
    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.inner.deserialize_any(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Seq(Some(len)),
        })
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.deserialize_tuple(len, visitor)
    }
    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, D::Error> {
        self.inner.deserialize_map(Wrapped {
            visitor,
            depth: self.depth,
            gate: Gate::Any,
        })
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.inner.deserialize_struct(
            name,
            fields,
            Wrapped {
                visitor,
                depth: self.depth,
                gate: Gate::Any,
            },
        )
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, D::Error> {
        self.inner.deserialize_enum(
            name,
            variants,
            Wrapped {
                visitor,
                depth: self.depth,
                gate: Gate::Any,
            },
        )
    }
    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}
