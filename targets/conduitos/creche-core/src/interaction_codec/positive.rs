//! Packed elements retain Ciborium's positive-integer visitor dispatch.
use core::marker::PhantomData;
use serde::de::{self, Visitor};
pub(super) struct Positive<E>(pub u8, pub PhantomData<E>);
impl<'de, E: de::Error> de::Deserializer<'de> for Positive<E> {
    type Error = E;
    fn deserialize_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_u64(self.0.into())
    }
    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_u64(u64::from(self.0))
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_u64(u64::from(self.0))
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_u64(u64::from(self.0))
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_u64(u64::from(self.0))
    }
    fn deserialize_u128<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_u128(u128::from(self.0))
    }
    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_i64(i64::from(self.0))
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_i64(i64::from(self.0))
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_i64(i64::from(self.0))
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_i64(i64::from(self.0))
    }
    fn deserialize_i128<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_i128(i128::from(self.0))
    }
    fn deserialize_option<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        v.visit_some(self)
    }
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        v: V,
    ) -> Result<V::Value, E> {
        v.visit_newtype_struct(self)
    }
    fn deserialize_ignored_any<V: Visitor<'de>>(self, v: V) -> Result<V::Value, E> {
        self.deserialize_any(v)
    }
    fn is_human_readable(&self) -> bool {
        false
    }
    fn deserialize_bool<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_f32<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_f64<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_char<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_str<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_string<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_bytes<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_unit<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_seq<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_map<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_identifier<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: V,
    ) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, _: V) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        _: V,
    ) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, E> {
        Err(E::custom("expected non-integer"))
    }
}
