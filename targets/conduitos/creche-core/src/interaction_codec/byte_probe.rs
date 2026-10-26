//! Probe only the genuine serde u8 method; never inspect nested values.
use core::fmt;
use serde::{
    Serialize, Serializer,
    ser::{self, Impossible},
};
#[derive(Debug)]
struct NotByte;
impl fmt::Display for NotByte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a scalar byte")
    }
}
impl core::error::Error for NotByte {}
impl ser::Error for NotByte {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Self
    }
}
pub(super) fn byte<T: Serialize + ?Sized>(value: &T) -> Option<u8> {
    value.serialize(Probe).ok().flatten()
}
struct Probe;
impl Serializer for Probe {
    type Ok = Option<u8>;
    type Error = NotByte;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;
    fn is_human_readable(&self) -> bool {
        false
    }
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value))
    }
    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        let _ = value;
        Ok(None)
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(NotByte)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(NotByte)
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(NotByte)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(NotByte)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(NotByte)
    }
    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(NotByte)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(NotByte)
    }
}
