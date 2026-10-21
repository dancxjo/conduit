//! Private no-allocation Serde JSON backend with checked errors, never panics
//! for unsupported Serde forms. The framing adapter remains its only caller.
mod compound;
mod writer;
use compound::Compound;
use serde::{Serialize, Serializer, ser::*};
use writer::{Error, Writer};

pub(super) fn encode<T: Serialize + ?Sized>(
    value: &T,
    output: &mut [u8],
) -> Result<usize, super::Refusal> {
    let mut writer = Writer {
        output,
        at: 0,
        key: false,
    };
    value.serialize(&mut writer).map_err(|error| match error {
        Error::Capacity => super::Refusal::Capacity,
        Error::Malformed => super::Refusal::Malformed,
    })?;
    Ok(writer.at)
}
impl<'b> Writer<'b> {
    fn array<'a>(&'a mut self) -> Result<Compound<'a, 'b>, Error> {
        self.compound(b"[")?;
        Ok(Compound {
            writer: self,
            first: true,
            close: b"]",
            awaiting_value: false,
        })
    }
    fn object<'a>(&'a mut self) -> Result<Compound<'a, 'b>, Error> {
        self.compound(b"{")?;
        Ok(Compound {
            writer: self,
            first: true,
            close: b"}",
            awaiting_value: false,
        })
    }
    fn variant(&mut self, variant: &str) -> Result<(), Error> {
        self.compound(b"{")?;
        self.string(variant)?;
        self.byte(b':')
    }
}
macro_rules! integer {($($method:ident($ty:ty)),*$(,)?)=>{$(
    fn $method(self,value:$ty)->Result<(),Error>{self.number(value)}
)*};}
impl<'a, 'b> Serializer for &'a mut Writer<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Compound<'a, 'b>;
    type SerializeTuple = Compound<'a, 'b>;
    type SerializeTupleStruct = Compound<'a, 'b>;
    type SerializeTupleVariant = Compound<'a, 'b>;
    type SerializeMap = Compound<'a, 'b>;
    type SerializeStruct = Compound<'a, 'b>;
    type SerializeStructVariant = Compound<'a, 'b>;
    fn serialize_bool(self, value: bool) -> Result<(), Error> {
        if self.key {
            self.string(if value { "true" } else { "false" })
        } else {
            self.bytes(if value { b"true" } else { b"false" })
        }
    }
    integer!(
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128)
    );
    fn serialize_f32(self, value: f32) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::Malformed);
        }
        if value == 0.0 {
            return if self.key {
                self.string(if value.is_sign_negative() {
                    "-0.0"
                } else {
                    "0.0"
                })
            } else {
                self.bytes(if value.is_sign_negative() {
                    b"-0.0"
                } else {
                    b"0.0"
                })
            };
        }
        self.scientific(value)
    }
    fn serialize_f64(self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::Malformed);
        }
        if value == 0.0 {
            return if self.key {
                self.string(if value.is_sign_negative() {
                    "-0.0"
                } else {
                    "0.0"
                })
            } else {
                self.bytes(if value.is_sign_negative() {
                    b"-0.0"
                } else {
                    b"0.0"
                })
            };
        }
        self.scientific(value)
    }
    fn serialize_char(self, value: char) -> Result<(), Error> {
        self.string(value.encode_utf8(&mut [0; 4]))
    }
    fn serialize_str(self, value: &str) -> Result<(), Error> {
        self.string(value)
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<(), Error> {
        let mut sequence = self.serialize_seq(Some(value.len()))?;
        for byte in value {
            SerializeSeq::serialize_element(&mut sequence, byte)?;
        }
        SerializeSeq::end(sequence)
    }
    fn serialize_none(self) -> Result<(), Error> {
        self.serialize_unit()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        if self.key {
            return Err(Error::Malformed);
        }
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        if self.key {
            return Err(Error::Malformed);
        }
        self.bytes(b"null")
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.string(variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.variant(variant)?;
        value.serialize(&mut *self)?;
        self.byte(b'}')
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        self.array()
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Error> {
        self.array()
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        self.array()
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        self.variant(variant)?;
        self.byte(b'[')?;
        Ok(Compound {
            writer: self,
            first: true,
            close: b"]}",
            awaiting_value: false,
        })
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        self.object()
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Error> {
        self.object()
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        self.variant(variant)?;
        self.byte(b'{')?;
        Ok(Compound {
            writer: self,
            first: true,
            close: b"}}",
            awaiting_value: false,
        })
    }
    fn collect_str<T: core::fmt::Display + ?Sized>(self, value: &T) -> Result<(), Error> {
        self.quoted(value)
    }
}
