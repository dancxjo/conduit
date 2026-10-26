//! Recursive private serializer; only genuine nonempty u8 sequences pack.
use super::byte_probe::byte;
use alloc::vec::Vec;
use serde::{
    Serialize, Serializer,
    ser::{
        self, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
};
pub(super) struct Packed<S>(pub(super) S);
pub(super) struct Ref<'a, T: ?Sized>(pub(super) &'a T);
impl<T: Serialize + ?Sized> Serialize for Ref<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(Packed(s))
    }
}
impl<S: Serializer> Serializer for Packed<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Sequence<S>;
    type SerializeTuple = Tuple<S>;
    type SerializeTupleStruct = TupleStruct<S>;
    type SerializeTupleVariant = Variant<S::SerializeTupleVariant>;
    type SerializeMap = Map<S::SerializeMap>;
    type SerializeStruct = Struct<S::SerializeStruct>;
    type SerializeStructVariant = StructVariant<S::SerializeStructVariant>;
    fn is_human_readable(&self) -> bool {
        false
    }
    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_bool(v)
    }
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i8(v)
    }
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i16(v)
    }
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i32(v)
    }
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i64(v)
    }
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u8(v)
    }
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u16(v)
    }
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u32(v)
    }
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u64(v)
    }
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_f32(v)
    }
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_f64(v)
    }
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_char(v)
    }
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_str(v)
    }
    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u64(
            u64::try_from(v).map_err(|_| ser::Error::custom("private integer capacity"))?,
        )
    }
    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        if v >= 0 {
            self.serialize_u128(v as u128)
        } else if u64::try_from(-1 - v).is_ok() {
            self.0.serialize_i128(v)
        } else {
            Err(ser::Error::custom("private integer capacity"))
        }
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(ser::Error::custom(
            "native byte strings are not private sequences",
        ))
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_some(&Ref(v))
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }
    fn serialize_unit_struct(self, n: &'static str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(n)
    }
    fn serialize_unit_variant(
        self,
        n: &'static str,
        i: u32,
        v: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(n, i, v)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        n: &'static str,
        v: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_struct(n, &Ref(v))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        n: &'static str,
        i: u32,
        k: &'static str,
        v: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_variant(n, i, k, &Ref(v))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(Sequence {
            serializer: Some(self.0),
            active: None,
            bytes: Vec::new(),
            len,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(Tuple {
            serializer: Some(self.0),
            active: None,
            bytes: Vec::new(),
            len,
        })
    }
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(TupleStruct {
            serializer: Some(self.0),
            active: None,
            bytes: Vec::new(),
            len,
            name,
        })
    }
    fn serialize_tuple_variant(
        self,
        n: &'static str,
        i: u32,
        v: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.0.serialize_tuple_variant(n, i, v, len).map(Variant)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.0.serialize_map(len).map(Map)
    }
    fn serialize_struct(
        self,
        n: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.0.serialize_struct(n, len).map(Struct)
    }
    fn serialize_struct_variant(
        self,
        n: &'static str,
        i: u32,
        v: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.0
            .serialize_struct_variant(n, i, v, len)
            .map(StructVariant)
    }
}
pub(super) struct Sequence<S: Serializer> {
    serializer: Option<S>,
    active: Option<S::SerializeSeq>,
    bytes: Vec<u8>,
    len: Option<usize>,
}
impl<S: Serializer> Sequence<S> {
    fn open(&mut self) -> Result<(), S::Error> {
        if self.active.is_none() {
            let mut active = self
                .serializer
                .take()
                .ok_or_else(|| ser::Error::custom("private sequence state"))?
                .serialize_seq(self.len)?;
            for value in &self.bytes {
                active.serialize_element(value)?;
            }
            self.bytes.clear();
            self.active = Some(active);
        }
        Ok(())
    }
}
impl<S: Serializer> SerializeSeq for Sequence<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), S::Error> {
        if self.active.is_none()
            && let Some(value) = byte(value)
        {
            if self.bytes.len() >= crate::wire::MAX_BLOB_BYTES as usize {
                return Err(ser::Error::custom("private sequence capacity"));
            }
            self.bytes
                .try_reserve(1)
                .map_err(|_| ser::Error::custom("private sequence capacity"))?;
            self.bytes.push(value);
            return Ok(());
        }
        self.open()?;
        self.active
            .as_mut()
            .ok_or_else(|| ser::Error::custom("private sequence state"))?
            .serialize_element(&Ref(value))
    }
    fn end(mut self) -> Result<S::Ok, S::Error> {
        if self.active.is_none() && self.len.is_some_and(|len| len != self.bytes.len()) {
            return Err(ser::Error::custom("private sequence declared length"));
        }
        if self.active.is_none() && !self.bytes.is_empty() {
            return self
                .serializer
                .take()
                .ok_or_else(|| ser::Error::custom("private sequence state"))?
                .serialize_bytes(&self.bytes);
        }
        self.open()?;
        self.active
            .take()
            .ok_or_else(|| ser::Error::custom("private sequence state"))?
            .end()
    }
}
pub(super) struct Tuple<S: Serializer> {
    serializer: Option<S>,
    active: Option<S::SerializeTuple>,
    bytes: Vec<u8>,
    len: usize,
}
impl<S: Serializer> Tuple<S> {
    fn open(&mut self) -> Result<(), S::Error> {
        if self.active.is_none() {
            let mut active = self
                .serializer
                .take()
                .ok_or_else(|| ser::Error::custom("private sequence state"))?
                .serialize_tuple(self.len)?;
            for value in &self.bytes {
                active.serialize_element(value)?;
            }
            self.bytes.clear();
            self.active = Some(active);
        }
        Ok(())
    }
}
impl<S: Serializer> SerializeTuple for Tuple<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), S::Error> {
        if self.active.is_none()
            && let Some(value) = byte(value)
        {
            if self.bytes.len() >= crate::wire::MAX_BLOB_BYTES as usize {
                return Err(ser::Error::custom("private sequence capacity"));
            }
            self.bytes
                .try_reserve(1)
                .map_err(|_| ser::Error::custom("private sequence capacity"))?;
            self.bytes.push(value);
            return Ok(());
        }
        self.open()?;
        self.active
            .as_mut()
            .ok_or_else(|| ser::Error::custom("private sequence state"))?
            .serialize_element(&Ref(value))
    }
    fn end(mut self) -> Result<S::Ok, S::Error> {
        if self.active.is_none() && self.len != self.bytes.len() {
            return Err(ser::Error::custom("private sequence declared length"));
        }
        if self.active.is_none() && !self.bytes.is_empty() {
            return self
                .serializer
                .take()
                .ok_or_else(|| ser::Error::custom("private sequence state"))?
                .serialize_bytes(&self.bytes);
        }
        self.open()?;
        self.active
            .take()
            .ok_or_else(|| ser::Error::custom("private sequence state"))?
            .end()
    }
}
pub(super) struct TupleStruct<S: Serializer> {
    serializer: Option<S>,
    active: Option<S::SerializeTupleStruct>,
    bytes: Vec<u8>,
    len: usize,
    name: &'static str,
}
impl<S: Serializer> TupleStruct<S> {
    fn open(&mut self) -> Result<(), S::Error> {
        if self.active.is_none() {
            let mut active = self
                .serializer
                .take()
                .ok_or_else(|| ser::Error::custom("private sequence state"))?
                .serialize_tuple_struct(self.name, self.len)?;
            for value in &self.bytes {
                active.serialize_field(value)?;
            }
            self.bytes.clear();
            self.active = Some(active);
        }
        Ok(())
    }
}
impl<S: Serializer> SerializeTupleStruct for TupleStruct<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), S::Error> {
        if self.active.is_none()
            && let Some(value) = byte(value)
        {
            if self.bytes.len() >= crate::wire::MAX_BLOB_BYTES as usize {
                return Err(ser::Error::custom("private sequence capacity"));
            }
            self.bytes
                .try_reserve(1)
                .map_err(|_| ser::Error::custom("private sequence capacity"))?;
            self.bytes.push(value);
            return Ok(());
        }
        self.open()?;
        self.active
            .as_mut()
            .ok_or_else(|| ser::Error::custom("private sequence state"))?
            .serialize_field(&Ref(value))
    }
    fn end(mut self) -> Result<S::Ok, S::Error> {
        if self.active.is_none() && self.len != self.bytes.len() {
            return Err(ser::Error::custom("private sequence declared length"));
        }
        if self.active.is_none() && !self.bytes.is_empty() {
            return self
                .serializer
                .take()
                .ok_or_else(|| ser::Error::custom("private sequence state"))?
                .serialize_bytes(&self.bytes);
        }
        self.open()?;
        self.active
            .take()
            .ok_or_else(|| ser::Error::custom("private sequence state"))?
            .end()
    }
}
pub(super) struct Variant<A>(A);
impl<A: SerializeTupleVariant> SerializeTupleVariant for Variant<A> {
    type Ok = A::Ok;
    type Error = A::Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), A::Error> {
        self.0.serialize_field(&Ref(v))
    }
    fn end(self) -> Result<A::Ok, A::Error> {
        self.0.end()
    }
}
pub(super) struct Map<A>(A);
impl<A: SerializeMap> SerializeMap for Map<A> {
    type Ok = A::Ok;
    type Error = A::Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), A::Error> {
        self.0.serialize_key(&Ref(v))
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<(), A::Error> {
        self.0.serialize_value(&Ref(v))
    }
    fn end(self) -> Result<A::Ok, A::Error> {
        self.0.end()
    }
}
pub(super) struct Struct<A>(A);
impl<A: SerializeStruct> SerializeStruct for Struct<A> {
    type Ok = A::Ok;
    type Error = A::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        k: &'static str,
        v: &T,
    ) -> Result<(), A::Error> {
        self.0.serialize_field(k, &Ref(v))
    }
    fn skip_field(&mut self, k: &'static str) -> Result<(), A::Error> {
        self.0.skip_field(k)
    }
    fn end(self) -> Result<A::Ok, A::Error> {
        self.0.end()
    }
}
pub(super) struct StructVariant<A>(A);
impl<A: SerializeStructVariant> SerializeStructVariant for StructVariant<A> {
    type Ok = A::Ok;
    type Error = A::Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        k: &'static str,
        v: &T,
    ) -> Result<(), A::Error> {
        self.0.serialize_field(k, &Ref(v))
    }
    fn skip_field(&mut self, k: &'static str) -> Result<(), A::Error> {
        self.0.skip_field(k)
    }
    fn end(self) -> Result<A::Ok, A::Error> {
        self.0.end()
    }
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
