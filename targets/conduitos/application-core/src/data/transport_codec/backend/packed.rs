//! Private byte-only base64 string spelling; no temporary byte/string backing.
use super::{Error, Writer};
use serde::{Serialize, Serializer, ser::*};
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub(super) fn encode<T: Serialize + ?Sized>(
    value: &T,
    writer: &mut Writer<'_>,
) -> Result<(), Error> {
    writer.bytes(b"\"~b64:")?;
    let mut state = State {
        writer,
        expected: None,
        seen: 0,
        pending: [0; 3],
        used: 0,
    };
    value.serialize(Collect(&mut state))?;
    state.writer.byte(b'"')
}
struct State<'a, 'b> {
    writer: &'a mut Writer<'b>,
    expected: Option<usize>,
    seen: usize,
    pending: [u8; 3],
    used: usize,
}
impl State<'_, '_> {
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        if self.expected.is_none_or(|n| self.seen >= n) {
            return Err(Error::Malformed);
        }
        self.pending[self.used] = byte;
        self.used += 1;
        self.seen += 1;
        if self.used == 3 {
            self.flush()?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<(), Error> {
        let [a, b, c] = self.pending;
        self.writer.bytes(&[
            ALPHABET[(a >> 2) as usize],
            ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize],
            if self.used > 1 {
                ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize]
            } else {
                b'='
            },
            if self.used > 2 {
                ALPHABET[(c & 63) as usize]
            } else {
                b'='
            },
        ])?;
        self.pending = [0; 3];
        self.used = 0;
        Ok(())
    }
}
struct Collect<'a, 'b, 'c>(&'a mut State<'b, 'c>);
macro_rules! reject {($($name:ident($($arg:ident:$ty:ty),*)),*$(,)?)=>{$(fn $name(self,$($arg:$ty),*)->Result<(),Error>{ $(let _ = $arg;)* Err(Error::Malformed)})*};}
impl<'a, 'b, 'c> Serializer for Collect<'a, 'b, 'c> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;
    fn serialize_u8(self, value: u8) -> Result<(), Error> {
        self.0.byte(value)
    }
    reject!(serialize_bool(value:bool),serialize_i8(value:i8),serialize_i16(value:i16),serialize_i32(value:i32),serialize_i64(value:i64),serialize_i128(value:i128),serialize_u16(value:u16),serialize_u32(value:u32),serialize_u64(value:u64),serialize_u128(value:u128),serialize_f32(value:f32),serialize_f64(value:f64),serialize_char(value:char),serialize_str(value:&str),serialize_unit(),serialize_unit_struct(name:&'static str),serialize_unit_variant(name:&'static str,index:u32,variant:&'static str));
    fn serialize_none(self) -> Result<(), Error> {
        Err(Error::Malformed)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<(), Error> {
        Err(Error::Malformed)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<(), Error> {
        Err(Error::Malformed)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), Error> {
        Err(Error::Malformed)
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<(), Error> {
        Err(Error::Malformed)
    }
    fn serialize_seq(self, length: Option<usize>) -> Result<Self, Error> {
        let length = length.ok_or(Error::Malformed)?;
        if self.0.expected.is_some() || length > super::super::packed::MAX_BYTES {
            return Err(Error::Malformed);
        }
        // Full encoded extent must fit the existing output slice before any element.
        let encoded = length
            .checked_add(2)
            .and_then(|n| (n / 3).checked_mul(4))
            .ok_or(Error::Capacity)?;
        if encoded
            .checked_add(1)
            .is_none_or(|n| n > self.0.writer.output.len() - self.0.writer.at)
        {
            return Err(Error::Capacity);
        }
        self.0.expected = Some(length);
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Error> {
        Err(Error::Malformed)
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(Error::Malformed)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(Error::Malformed)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(Error::Malformed)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Error> {
        Err(Error::Malformed)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(Error::Malformed)
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}
impl SerializeSeq for Collect<'_, '_, '_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        value.serialize(Collect(self.0))
    }
    fn end(self) -> Result<(), Error> {
        if self.0.expected != Some(self.0.seen) {
            return Err(Error::Malformed);
        }
        if self.0.used != 0 {
            self.0.flush()?;
        }
        Ok(())
    }
}
