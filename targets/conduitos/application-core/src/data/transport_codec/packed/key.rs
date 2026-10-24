//! Observe decoded map keys while preserving their original typed visitor.
use core::fmt;
use serde::de::*;
pub(in super::super) struct KeySeed<'a, S> {
    pub(in super::super) seed: S,
    pub(in super::super) packed: &'a mut bool,
}
impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for KeySeed<'_, S> {
    type Value = S::Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.seed.deserialize(Capture(d, self.packed))
    }
}
struct Capture<'a, D>(D, &'a mut bool);
struct KeyVisitor<'a, V>(V, &'a mut bool);
macro_rules! forward {($($method:ident($($arg:ident:$ty:ty),*)),*$(,)?)=>{$(
fn $method<V:Visitor<'de>>(self,$($arg:$ty,)*visitor:V)->Result<V::Value,Self::Error>{self.0.$method($($arg,)*KeyVisitor(visitor,self.1))}
)*};}
impl<'de, D: Deserializer<'de>> Deserializer<'de> for Capture<'_, D> {
    type Error = D::Error;
    forward!(deserialize_any(),deserialize_bool(),deserialize_i8(),deserialize_i16(),deserialize_i32(),deserialize_i64(),deserialize_i128(),deserialize_u8(),deserialize_u16(),deserialize_u32(),deserialize_u64(),deserialize_u128(),deserialize_f32(),deserialize_f64(),deserialize_char(),deserialize_str(),deserialize_string(),deserialize_bytes(),deserialize_byte_buf(),deserialize_option(),deserialize_unit(),deserialize_unit_struct(name:&'static str),deserialize_newtype_struct(name:&'static str),deserialize_seq(),deserialize_tuple(len:usize),deserialize_tuple_struct(name:&'static str,len:usize),deserialize_map(),deserialize_struct(name:&'static str,fields:&'static[&'static str]),deserialize_enum(name:&'static str,variants:&'static[&'static str]),deserialize_identifier(),deserialize_ignored_any());
    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}
macro_rules! scalar {($($method:ident($ty:ty)),*$(,)?)=>{$(fn $method<E:Error>(self,value:$ty)->Result<Self::Value,E>{self.0.$method(value)})*};}
impl<'de, V: Visitor<'de>> Visitor<'de> for KeyVisitor<'_, V> {
    type Value = V::Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.expecting(f)
    }
    scalar!(
        visit_bool(bool),
        visit_i8(i8),
        visit_i16(i16),
        visit_i32(i32),
        visit_i64(i64),
        visit_i128(i128),
        visit_u8(u8),
        visit_u16(u16),
        visit_u32(u32),
        visit_u64(u64),
        visit_u128(u128),
        visit_f32(f32),
        visit_f64(f64),
        visit_char(char),
        visit_bytes(&[u8]),
        visit_borrowed_bytes(&'de [u8]),
        visit_byte_buf(alloc::vec::Vec<u8>)
    );
    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        *self.1 = v == "canonical_value";
        self.0.visit_str(v)
    }
    fn visit_borrowed_str<E: Error>(self, v: &'de str) -> Result<Self::Value, E> {
        *self.1 = v == "canonical_value";
        self.0.visit_borrowed_str(v)
    }
    fn visit_string<E: Error>(self, v: alloc::string::String) -> Result<Self::Value, E> {
        *self.1 = v == "canonical_value";
        self.0.visit_string(v)
    }
    fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
        self.0.visit_unit()
    }
    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        self.0.visit_none()
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.0.visit_some(d)
    }
    fn visit_newtype_struct<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.0.visit_newtype_struct(d)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        self.0.visit_seq(a)
    }
    fn visit_map<A: MapAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        self.0.visit_map(a)
    }
    fn visit_enum<A: EnumAccess<'de>>(self, a: A) -> Result<Self::Value, A::Error> {
        self.0.visit_enum(a)
    }
}
